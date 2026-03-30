# Hunyuan 3D is licensed under the TENCENT HUNYUAN NON-COMMERCIAL LICENSE AGREEMENT
# except for the third-party components listed below.
# Hunyuan 3D does not impose any additional limitations beyond what is outlined
# in the repsective licenses of these third-party components.
# Users must comply with all terms and conditions of original licenses of these third-party
# components and must ensure that the usage of the third party components adheres to
# all relevant laws and regulations.

# For avoidance of doubts, Hunyuan 3D means the large language models and
# their software and algorithms, including trained model weights, parameters (including
# optimizer states), machine-learning model code, inference-enabling code, training-enabling code,
# fine-tuning enabling code and other elements of the foregoing made publicly available
# by Tencent in accordance with TENCENT HUNYUAN COMMUNITY LICENSE AGREEMENT.

import os
import torch
import random
import numpy as np
from PIL import Image
from typing import List
import huggingface_hub
from omegaconf import OmegaConf
from diffusers import DiffusionPipeline
from diffusers import EulerAncestralDiscreteScheduler, DDIMScheduler, UniPCMultistepScheduler


def _quantize_model_nf4(model: "torch.nn.Module") -> "torch.nn.Module":
    """Substitui nn.Linear por Linear4bit (NF4) in-place — reduz ~75% de VRAM dos pesos."""
    try:
        import bitsandbytes as bnb
    except ImportError:
        return model

    for name, module in list(model.named_modules()):
        if not isinstance(module, torch.nn.Linear):
            continue
        parts = name.rsplit(".", 1)
        parent = model if len(parts) == 1 else dict(model.named_modules())[parts[0]]
        attr = parts[-1] if len(parts) > 1 else parts[0]

        quant_linear = bnb.nn.Linear4bit(
            module.in_features,
            module.out_features,
            bias=module.bias is not None,
            compute_dtype=torch.float16,
            quant_type="nf4",
        )
        quant_linear.weight = bnb.nn.Params4bit(
            module.weight.data,
            requires_grad=False,
            quant_type="nf4",
            compress_statistics=False,
        )
        if module.bias is not None:
            quant_linear.bias = module.bias
        quant_linear = quant_linear.to(module.weight.device)
        setattr(parent, attr, quant_linear)

    return model


class multiviewDiffusionNet:
    def __init__(self, config) -> None:
        self.device = config.device

        cfg_path = config.multiview_cfg_path
        custom_pipeline = os.path.join(os.path.dirname(__file__), "..", "hunyuanpaintpbr")
        cfg = OmegaConf.load(cfg_path)
        self.cfg = cfg
        self.mode = self.cfg.model.params.stable_diffusion_config.custom_pipeline[2:]

        weights_subfolder = getattr(
            config, "multiview_weights_subfolder", "hunyuan3d-paintpbr-v2-1"
        )
        model_path = huggingface_hub.snapshot_download(
            repo_id=config.multiview_pretrained_path,
            allow_patterns=[f"{weights_subfolder}/*"],
        )
        model_path = os.path.join(model_path, weights_subfolder)

        pipeline = DiffusionPipeline.from_pretrained(
            model_path,
            custom_pipeline=custom_pipeline,
            torch_dtype=torch.float16,
        )

        pipeline.scheduler = UniPCMultistepScheduler.from_config(
            pipeline.scheduler.config, timestep_spacing="trailing"
        )
        pipeline.set_progress_bar_config(disable=True)
        pipeline.eval()
        setattr(pipeline, "view_size", cfg.model.params.get("view_size", 320))

        self.pipeline = pipeline.to(self.device)

        if hasattr(self.pipeline, "unet") and self.pipeline.unet is not None:
            _quantize_model_nf4(self.pipeline.unet)
        if hasattr(self.pipeline, "text_encoder") and self.pipeline.text_encoder is not None:
            _quantize_model_nf4(self.pipeline.text_encoder)
        torch.cuda.empty_cache()

        if hasattr(self.pipeline, "enable_vae_slicing"):
            self.pipeline.enable_vae_slicing()
        if hasattr(self.pipeline, "enable_vae_tiling"):
            self.pipeline.enable_vae_tiling()

        # DINO sempre em CPU: ~1.1 GB fp16 que não cabem na GPU com UNet.
        # O output é movido para CUDA no forward (~ms de overhead).
        self._dino_on_cpu = True
        if hasattr(self.pipeline.unet, "use_dino") and self.pipeline.unet.use_dino:
            from ..hunyuanpaintpbr.unet.modules import Dino_v2

            self.dino_v2 = Dino_v2(config.dino_ckpt_path).to(torch.float16).to("cpu")

    def seed_everything(self, seed):
        random.seed(seed)
        np.random.seed(seed)
        torch.manual_seed(seed)
        os.environ["PL_GLOBAL_SEED"] = str(seed)

    @torch.no_grad()
    def __call__(self, images, conditions, prompt=None, custom_view_size=None, resize_input=False):
        pils = self.forward_one(
            images, conditions, prompt=prompt, custom_view_size=custom_view_size, resize_input=resize_input
        )
        return pils

    def forward_one(self, input_images, control_images, prompt=None, custom_view_size=None, resize_input=False):
        self.seed_everything(0)
        custom_view_size = custom_view_size if custom_view_size is not None else self.pipeline.view_size
        if not isinstance(input_images, List):
            input_images = [input_images]
        if not resize_input:
            input_images = [
                input_image.resize((self.pipeline.view_size, self.pipeline.view_size)) for input_image in input_images
            ]
        else:
            input_images = [input_image.resize((custom_view_size, custom_view_size)) for input_image in input_images]
        for i in range(len(control_images)):
            control_images[i] = control_images[i].resize((custom_view_size, custom_view_size))
            if control_images[i].mode == "L":
                control_images[i] = control_images[i].point(lambda x: 255 if x > 1 else 0, mode="1")

        kwargs = dict(generator=torch.Generator(device=self.pipeline.device).manual_seed(0))

        num_view = len(control_images) // 2
        normal_image = [[control_images[i] for i in range(num_view)]]
        position_image = [[control_images[i + num_view] for i in range(num_view)]]

        kwargs["width"] = custom_view_size
        kwargs["height"] = custom_view_size
        kwargs["num_in_batch"] = num_view
        kwargs["images_normal"] = normal_image
        kwargs["images_position"] = position_image

        if hasattr(self.pipeline.unet, "use_dino") and self.pipeline.unet.use_dino:
            dino_hidden_states = self.dino_v2(input_images[0])
            if self._dino_on_cpu and isinstance(dino_hidden_states, torch.Tensor):
                dino_hidden_states = dino_hidden_states.to(torch.device(self.device))
            kwargs["dino_hidden_states"] = dino_hidden_states

        sync_condition = None

        infer_steps_dict = {
            "EulerAncestralDiscreteScheduler": 30,
            "UniPCMultistepScheduler": 15,
            "DDIMScheduler": 50,
            "ShiftSNRScheduler": 15,
        }

        mvd_image = self.pipeline(
            input_images[0:1],
            num_inference_steps=infer_steps_dict[self.pipeline.scheduler.__class__.__name__],
            prompt=prompt,
            sync_condition=sync_condition,
            guidance_scale=3.0,
            **kwargs,
        ).images

        if "pbr" in self.mode:
            mvd_image = {"albedo": mvd_image[:num_view], "mr": mvd_image[num_view:]}
        else:
            mvd_image = {"hdr": mvd_image}

        return mvd_image
