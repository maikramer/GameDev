# Copyright (c) Real-ESRGAN / XPixel Group. SPDX-License-Identifier: BSD-3-Clause / Apache-2.0
# Vendido em Paint3D: o pacote PyPI ``realesrgan`` importa ``basicsr`` no ``__init__`` (archs),
# o que obriga a instalar basicsr. Esta cópia só de ``RealESRGANer`` + ``load_file_from_url``
# (de basicsr.utils.download_util) evita essa dependência.

from __future__ import annotations

import math
import os
import queue
import threading
from urllib.parse import urlparse

import cv2
import numpy as np
import torch
from torch.hub import download_url_to_file, get_dir
from torch.nn import functional as F

_PKG_DIR = os.path.dirname(os.path.abspath(__file__))
_WEIGHTS_DIR = os.path.join(_PKG_DIR, "realesrgan_weights")


def load_file_from_url(url: str, model_dir: str | None = None, progress: bool = True, file_name: str | None = None) -> str:
    if model_dir is None:
        hub_dir = get_dir()
        model_dir = os.path.join(hub_dir, "checkpoints")
    os.makedirs(model_dir, exist_ok=True)
    parts = urlparse(url)
    filename = os.path.basename(parts.path)
    if file_name is not None:
        filename = file_name
    cached_file = os.path.abspath(os.path.join(model_dir, filename))
    if not os.path.exists(cached_file):
        print(f'Downloading: "{url}" to {cached_file}\n')
        download_url_to_file(url, cached_file, hash_prefix=None, progress=progress)
    return cached_file


def _torch_load_checkpoint(path: str):
    """Carrega checkpoint RealESRGAN (pickle); PyTorch 2.6+ exige weights_only explícito."""
    kw: dict = {"map_location": torch.device("cpu")}
    try:
        return torch.load(path, **kw, weights_only=False)
    except TypeError:
        return torch.load(path, **kw)


class RealESRGANer:
    """Cópia funcional de realesrgan.utils.RealESRGANer (Real-ESRGAN 0.3.x)."""

    def __init__(
        self,
        scale,
        model_path,
        dni_weight=None,
        model=None,
        tile=0,
        tile_pad=10,
        pre_pad=10,
        half=False,
        device=None,
        gpu_id=None,
    ):
        self.scale = scale
        self.tile_size = tile
        self.tile_pad = tile_pad
        self.pre_pad = pre_pad
        self.mod_scale = None
        self.half = half

        if gpu_id:
            self.device = (
                torch.device(f"cuda:{gpu_id}" if torch.cuda.is_available() else "cpu")
                if device is None
                else device
            )
        else:
            self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu") if device is None else device

        if isinstance(model_path, list):
            assert len(model_path) == len(dni_weight), "model_path and dni_weight should have the same length."
            loadnet = self.dni(model_path[0], model_path[1], dni_weight)
        else:
            mp = str(model_path)
            if mp.startswith("https://"):
                model_path = load_file_from_url(
                    url=mp,
                    model_dir=_WEIGHTS_DIR,
                    progress=True,
                    file_name=None,
                )
            loadnet = _torch_load_checkpoint(str(model_path))

        keyname = "params_ema" if "params_ema" in loadnet else "params"
        model.load_state_dict(loadnet[keyname], strict=True)

        model.eval()
        self.model = model.to(self.device)
        if self.half:
            self.model = self.model.half()

    def dni(self, net_a, net_b, dni_weight, key="params", loc="cpu"):
        net_a = _torch_load_checkpoint(net_a)
        net_b = _torch_load_checkpoint(net_b)
        for k, v_a in net_a[key].items():
            net_a[key][k] = dni_weight[0] * v_a + dni_weight[1] * net_b[key][k]
        return net_a

    def pre_process(self, img):
        img = torch.from_numpy(np.transpose(img, (2, 0, 1))).float()
        self.img = img.unsqueeze(0).to(self.device)
        if self.half:
            self.img = self.img.half()

        if self.pre_pad != 0:
            self.img = F.pad(self.img, (0, self.pre_pad, 0, self.pre_pad), "reflect")
        if self.scale == 2:
            self.mod_scale = 2
        elif self.scale == 1:
            self.mod_scale = 4
        if self.mod_scale is not None:
            self.mod_pad_h, self.mod_pad_w = 0, 0
            _, _, h, w = self.img.size()
            if h % self.mod_scale != 0:
                self.mod_pad_h = self.mod_scale - h % self.mod_scale
            if w % self.mod_scale != 0:
                self.mod_pad_w = self.mod_scale - w % self.mod_scale
            self.img = F.pad(self.img, (0, self.mod_pad_w, 0, self.mod_pad_h), "reflect")

    def process(self):
        self.output = self.model(self.img)

    def tile_process(self):
        batch, channel, height, width = self.img.shape
        output_height = height * self.scale
        output_width = width * self.scale
        output_shape = (batch, channel, output_height, output_width)

        self.output = self.img.new_zeros(output_shape)
        tiles_x = math.ceil(width / self.tile_size)
        tiles_y = math.ceil(height / self.tile_size)

        for y in range(tiles_y):
            for x in range(tiles_x):
                ofs_x = x * self.tile_size
                ofs_y = y * self.tile_size
                input_start_x = ofs_x
                input_end_x = min(ofs_x + self.tile_size, width)
                input_start_y = ofs_y
                input_end_y = min(ofs_y + self.tile_size, height)

                input_start_x_pad = max(input_start_x - self.tile_pad, 0)
                input_end_x_pad = min(input_end_x + self.tile_pad, width)
                input_start_y_pad = max(input_start_y - self.tile_pad, 0)
                input_end_y_pad = min(input_end_y + self.tile_pad, height)

                input_tile_width = input_end_x - input_start_x
                input_tile_height = input_end_y - input_start_y
                tile_idx = y * tiles_x + x + 1
                input_tile = self.img[:, :, input_start_y_pad:input_end_y_pad, input_start_x_pad:input_end_x_pad]

                try:
                    with torch.no_grad():
                        output_tile = self.model(input_tile)
                except RuntimeError as error:
                    print("Error", error)
                print(f"\tTile {tile_idx}/{tiles_x * tiles_y}")

                output_start_x = input_start_x * self.scale
                output_end_x = input_end_x * self.scale
                output_start_y = input_start_y * self.scale
                output_end_y = input_end_y * self.scale

                output_start_x_tile = (input_start_x - input_start_x_pad) * self.scale
                output_end_x_tile = output_start_x_tile + input_tile_width * self.scale
                output_start_y_tile = (input_start_y - input_start_y_pad) * self.scale
                output_end_y_tile = output_start_y_tile + input_tile_height * self.scale

                self.output[:, :, output_start_y:output_end_y, output_start_x:output_end_x] = output_tile[
                    :, :, output_start_y_tile:output_end_y_tile, output_start_x_tile:output_end_x_tile
                ]

    def post_process(self):
        if self.mod_scale is not None:
            _, _, h, w = self.output.size()
            self.output = self.output[:, :, 0 : h - self.mod_pad_h * self.scale, 0 : w - self.mod_pad_w * self.scale]
        if self.pre_pad != 0:
            _, _, h, w = self.output.size()
            self.output = self.output[:, :, 0 : h - self.pre_pad * self.scale, 0 : w - self.pre_pad * self.scale]
        return self.output

    @torch.no_grad()
    def enhance(self, img, outscale=None, alpha_upsampler="realesrgan"):
        h_input, w_input = img.shape[0:2]
        img = img.astype(np.float32)
        if np.max(img) > 256:
            max_range = 65535
            print("\tInput is a 16-bit image")
        else:
            max_range = 255
        img = img / max_range
        if len(img.shape) == 2:
            img_mode = "L"
            img = cv2.cvtColor(img, cv2.COLOR_GRAY2RGB)
        elif img.shape[2] == 4:
            img_mode = "RGBA"
            alpha = img[:, :, 3]
            img = img[:, :, 0:3]
            img = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)
            if alpha_upsampler == "realesrgan":
                alpha = cv2.cvtColor(alpha, cv2.COLOR_GRAY2RGB)
        else:
            img_mode = "RGB"
            img = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)

        self.pre_process(img)
        if self.tile_size > 0:
            self.tile_process()
        else:
            self.process()
        output_img = self.post_process()
        output_img = output_img.data.squeeze().float().cpu().clamp_(0, 1).numpy()
        output_img = np.transpose(output_img[[2, 1, 0], :, :], (1, 2, 0))
        if img_mode == "L":
            output_img = cv2.cvtColor(output_img, cv2.COLOR_BGR2GRAY)

        if img_mode == "RGBA":
            if alpha_upsampler == "realesrgan":
                self.pre_process(alpha)
                if self.tile_size > 0:
                    self.tile_process()
                else:
                    self.process()
                output_alpha = self.post_process()
                output_alpha = output_alpha.data.squeeze().float().cpu().clamp_(0, 1).numpy()
                output_alpha = np.transpose(output_alpha[[2, 1, 0], :, :], (1, 2, 0))
                output_alpha = cv2.cvtColor(output_alpha, cv2.COLOR_BGR2GRAY)
            else:
                h, w = alpha.shape[0:2]
                output_alpha = cv2.resize(alpha, (w * self.scale, h * self.scale), interpolation=cv2.INTER_LINEAR)

            output_img = cv2.cvtColor(output_img, cv2.COLOR_BGR2BGRA)
            output_img[:, :, 3] = output_alpha

        if max_range == 65535:
            output = (output_img * 65535.0).round().astype(np.uint16)
        else:
            output = (output_img * 255.0).round().astype(np.uint8)

        if outscale is not None and outscale != float(self.scale):
            output = cv2.resize(
                output,
                (int(w_input * outscale), int(h_input * outscale)),
                interpolation=cv2.INTER_LANCZOS4,
            )

        return output, img_mode


class PrefetchReader(threading.Thread):
    def __init__(self, img_list, num_prefetch_queue):
        super().__init__()
        self.que = queue.Queue(num_prefetch_queue)
        self.img_list = img_list

    def run(self):
        for img_path in self.img_list:
            img = cv2.imread(img_path, cv2.IMREAD_UNCHANGED)
            self.que.put(img)
        self.que.put(None)

    def __next__(self):
        next_item = self.que.get()
        if next_item is None:
            raise StopIteration
        return next_item

    def __iter__(self):
        return self


class IOConsumer(threading.Thread):
    def __init__(self, opt, que, qid):
        super().__init__()
        self._queue = que
        self.qid = qid
        self.opt = opt

    def run(self):
        while True:
            msg = self._queue.get()
            if isinstance(msg, str) and msg == "quit":
                break
            output = msg["output"]
            save_path = msg["save_path"]
            cv2.imwrite(save_path, output)
        print(f"IO worker {self.qid} is done.")
