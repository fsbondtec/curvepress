"""curvepress — lossy time series compression: RDP/VW + quantization."""
from ._curvepress import (
    compress,
    compress_stats,
    decompress,
    interpolate,
    version,
)

__all__ = ["compress", "compress_stats", "decompress", "interpolate", "version"]
__version__ = version()
