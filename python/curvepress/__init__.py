"""curvepress — lossy time series compression: RDP/VW + quantization."""
from ._curvepress import (
    compress_rdp,
    compress_rdp_stats,
    compress_vw,
    compress_vw_stats,
    compress_rdpn,
    compress_rdpn_stats,
    decompress,
    interpolate,
    version,
)

__all__ = [
    "compress_rdp",
    "compress_rdp_stats",
    "compress_vw",
    "compress_vw_stats",
    "compress_rdpn",
    "compress_rdpn_stats",
    "decompress",
    "interpolate",
    "version",
]
__version__ = version()
