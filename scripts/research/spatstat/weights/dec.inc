import struct
def dec(path):
    b = open(path,'rb').read()
    i = b.index(b'\n'); hdr = b[:i].decode(); p = i+1
    rows = []
    id64 = 'HASID64@True' in hdr
    if id64:
        n, rs = struct.unpack_from('<qq', b, p); p += 16
    else:
        n, rs = struct.unpack_from('<ii', b, p); p += 8
    for _ in range(n):
        if id64:
            uid, nn = struct.unpack_from('<qq', b, p); p += 16
        else:
            uid, nn = struct.unpack_from('<ii', b, p); p += 8
        if nn == 0:
            rows.append([uid, [], [], None]); continue
        if id64:
            nb = struct.unpack_from('<%dq' % nn, b, p); p += 8*nn
        else:
            nb = struct.unpack_from('<%di' % nn, b, p); p += 4*nn
        if 'FIXEDWEIGHTS@True' in hdr:
            w = list(struct.unpack_from('<d', b, p)); p += 8
        else:
            w = list(struct.unpack_from('<%dd' % nn, b, p)); p += 8*nn
        s = struct.unpack_from('<d', b, p)[0]; p += 8
        rows.append([uid, list(nb), w, s])
    return {'header': hdr, 'n': n, 'rowstd': rs, 'rows': rows, 'rest': len(b) - p, 'hex': b.hex()}
