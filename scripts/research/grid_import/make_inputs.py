"""Create intentional ASCII-grid and IEEE-float test files; no GDAL conversion."""
from pathlib import Path
import struct

folder = Path('testdata/raster/grid_import')
folder.mkdir(parents=True, exist_ok=True)
header = 'NCOLS 3\nNROWS 2\nXLLCORNER 500000\nYLLCORNER 5000000\nCELLSIZE 10\nNODATA_VALUE -9999\n'
rows = {
    'small': '-9999 0 1 127 128 255',
    'signed': '-9999 -6 -1 0 255 32767',
    'float': '-9999 -6.7 0.1 1.25 2.5 16777217.25',
    'wide': '-9999 -2147483648 2147483647 16777217 65535 3',
    'short': '1 2 3 4 5', 'extra': '1 2 3 4 5 6 7',
    'near': '-9999.0001 -9998.9999 -9999.1 0 1 2',
    'all_nd': '-9999 -9999 -9999 -9999 -9999 -9999',
}
for name, row in rows.items():
    (folder / (name + '.asc')).write_text(header + row + '\n')
(folder / 'center.asc').write_text(header.replace('XLLCORNER', 'XLLCENTER').replace('YLLCORNER', 'YLLCENTER') + rows['small'] + '\n')
(folder / 'no_nd.asc').write_text(header.replace('NODATA_VALUE -9999\n', '') + '1 2 3 4 -9999 6\n')
(folder / 'near_header.asc').write_text(header.replace('NODATA_VALUE -9999', 'NODATA_VALUE -9999.0001') + '-9999 -9999.0001 -9999.0002 1 2 3\n')
values = [-9999, -6.7, 0.1, 1.25, 2.5, 16777217.25]
little = struct.pack('<6f', *values)
standard_header = header + 'BYTEORDER LSBFIRST\n'
binary = {
    'little': (standard_header, little),
    'big': (header + 'BYTEORDER MSBFIRST\n', struct.pack('>6f', *values)),
    'float_no_byteorder': (header, little),
    'float_no_nd': (standard_header.replace('NODATA_VALUE -9999\n', ''), little),
    'float_center': (standard_header.replace('XLLCORNER', 'XLLCENTER').replace('YLLCORNER', 'YLLCENTER'), little),
    'float_near_header': (standard_header.replace('NODATA_VALUE -9999', 'NODATA_VALUE -9999.0001'), little),
    'float_short': (standard_header, little[:-4]),
    'float_extra': (standard_header, little + struct.pack('<f', 7.0)),
    'all_nd': (standard_header, struct.pack('<6f', *([-9999.0] * 6))),
}
for name, (hdr, data) in binary.items():
    (folder / (name + '.flt')).write_bytes(data)
    (folder / (name + '.hdr')).write_text(hdr)

(folder / "missing_header.flt").write_bytes(little)
(folder / "missing_header.hdr").unlink(missing_ok=True)
