"""
Text2D — Text-to-2D com FLUX.2 Klein 4B quantizado (SDNQ).
"""

__version__ = "0.1.0"
__author__ = "Text2D Project"

from .generator import KleinFluxGenerator, default_model_id

__all__ = ["KleinFluxGenerator", "default_model_id"]
