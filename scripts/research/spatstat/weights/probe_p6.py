import arcpy, json, random, struct, datetime
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/sw2'
sr = arcpy.SpatialReference(32632)
out = json.load(open(P(D) + '/out.json'))
random.seed(99)
# big point set with dates
bp = []
for i in range(90):
    bp.append((round(500000 + random.uniform(0, 2000), 3), round(5000000 + random.uniform(0, 1500), 3), round(random.uniform(0, 100), 3), random.randint(0, 60)))
buid = random.sample(range(1, 5000), len(bp))
fc = arcpy.management.CreateFeatureclass(P(D), 'big.shp', 'POINT', spatial_reference=sr)
arcpy.management.AddField(fc, 'UID', 'LONG'); arcpy.management.AddField(fc, 'V', 'DOUBLE'); arcpy.management.AddField(fc, 'T', 'DATE')
t0 = datetime.datetime(2024, 1, 1)
with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'UID', 'V', 'T']) as c:
    for i, (x, y, v, dd) in enumerate(bp): c.insertRow([(x, y), buid[i], v, t0 + datetime.timedelta(days=dd)])
out['big'] = [[x, y, v, dd, buid[i]] for i, (x, y, v, dd) in enumerate(bp)]
# big polygons 10x6 jittered, one cell split in two (T-junctions), one hole-less gap
nx, ny = 10, 6
node = {}
for i in range(nx+1):
    for j in range(ny+1):
        node[i,j] = (500000 + i*100 + (random.uniform(-25,25) if 0<i<nx else 0), 5000000 + j*100 + (random.uniform(-25,25) if 0<j<ny else 0))
pg = arcpy.management.CreateFeatureclass(P(D), 'bpg.shp', 'POLYGON', spatial_reference=sr)
arcpy.management.AddField(pg, 'UID', 'LONG'); arcpy.management.AddField(pg, 'V', 'DOUBLE')
polys = []
uids = random.sample(range(1, 3000), 80)
def add(c, ring):
    u = uids[len(polys)]; v = round(random.uniform(0, 50), 2)
    polys.append({'uid': u, 'ring': ring, 'v': v})
    c.insertRow([arcpy.Polygon(arcpy.Array([arcpy.Point(*p) for p in ring]), sr), u, v])
with arcpy.da.InsertCursor(pg, ['SHAPE@', 'UID', 'V']) as c:
    for j in range(ny):
        for i in range(nx):
            if (i, j) in [(4, 2), (7, 4)]: continue
            a, b, cc, dd = node[i,j], node[i,j+1], node[i+1,j+1], node[i+1,j]
            if (i, j) == (2, 3):
                # split vertically at midpoints: left half and right half
                m1 = ((a[0]+dd[0])/2, (a[1]+dd[1])/2); m2 = ((b[0]+cc[0])/2, (b[1]+cc[1])/2)
                add(c, [a, b, m2, m1, a]); add(c, [m1, m2, cc, dd, m1])
                continue
            add(c, [a, b, cc, dd, a])
out['bpolys'] = polys
def dec(path):
    b = open(path,'rb').read()
    i = b.index(b'\n'); hdr = b[:i].decode(); p = i+1
    n, rs = struct.unpack_from('<ii', b, p); p += 8
    rows = []
    id64 = 'HASID64@True' in hdr
    for _ in range(n):
        if id64:
            uid, nn = struct.unpack_from('<qi', b, p); p += 12
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
S = arcpy.stats
def go(name, src, *a, uid='UID', **kw):
    o = P(D) + '/' + name + '.swm'
    try:
        r = S.GenerateSpatialWeightsMatrix(src, uid, o, *a, **kw)
        out['swm'][name] = dec(o); out['swm'][name]['messages'] = r.getMessages()
        print('ok', name)
    except Exception as e:
        print('FAIL', name, str(e)[:300]); out['swm'][name] = {'error': str(e)}
bs, bg = P(D) + '/big.shp', P(D) + '/bpg.shp'
go('b_del', bs, 'DELAUNAY_TRIANGULATION', Row_Standardization='NO_STANDARDIZATION')
go('b_knn6', bs, 'K_NEAREST_NEIGHBORS', Number_of_Neighbors=6)
go('b_inv', bs, 'INVERSE_DISTANCE')
go('b_fix250', bs, 'FIXED_DISTANCE', Threshold_Distance=250, Row_Standardization='NO_STANDARDIZATION')
go('b_st', bs, 'SPACE_TIME_WINDOW', Threshold_Distance=400, Date_Time_Field='T', Date_Time_Interval_Type='DAYS', Date_Time_Interval_Value=10, Row_Standardization='NO_STANDARDIZATION')
go('b_st_std', bs, 'SPACE_TIME_WINDOW', Threshold_Distance=400, Date_Time_Field='T', Date_Time_Interval_Type='WEEKS', Date_Time_Interval_Value=1)
go('b_fid', bs, 'K_NEAREST_NEIGHBORS', Number_of_Neighbors=3, uid='FID')
go('bp_edges', bg, 'CONTIGUITY_EDGES_ONLY', Row_Standardization='NO_STANDARDIZATION')
go('bp_corners', bg, 'CONTIGUITY_EDGES_CORNERS', Row_Standardization='NO_STANDARDIZATION')
go('bp_edges_o2', bg, 'CONTIGUITY_EDGES_ONLY', order=2, Row_Standardization='NO_STANDARDIZATION')
go('bp_corners_o3', bg, 'CONTIGUITY_EDGES_CORNERS', order=3, Row_Standardization='NO_STANDARDIZATION')
go('bp_border', bg, 'CONTIGUITY_EDGES_CORNERS', weighting_method='BORDER', Row_Standardization='NO_STANDARDIZATION')
go('bp_del', bg, 'DELAUNAY_TRIANGULATION', Row_Standardization='NO_STANDARDIZATION')
go('bp_edges_focal', bg, 'CONTIGUITY_EDGES_ONLY', include_focal_feature='INCLUDE_FOCAL')
go('bp_inv_gauss', bg, 'INVERSE_DISTANCE', weighting_method='GAUSSIAN', Row_Standardization='NO_STANDARDIZATION')
go('bp_edges_gauss', bg, 'CONTIGUITY_EDGES_ONLY', weighting_method='GAUSSIAN', Row_Standardization='NO_STANDARDIZATION')
go('b_del_bisq_fix', bs, 'DELAUNAY_TRIANGULATION', weighting_method='BISQUARE', kernel_type='FIXED', Row_Standardization='NO_STANDARDIZATION')
go('b_del_bisq_ad', bs, 'DELAUNAY_TRIANGULATION', weighting_method='BISQUARE', kernel_type='ADAPTIVE', adaptive_neighbors=4, Row_Standardization='NO_STANDARDIZATION')
go('b_knn_bisq_fixdef', bs, 'K_NEAREST_NEIGHBORS', Number_of_Neighbors=6, weighting_method='BISQUARE', kernel_type='FIXED', Row_Standardization='NO_STANDARDIZATION')
# gdb with big integer id
gdb = P(D) + '/w.gdb'
if not arcpy.Exists(gdb): arcpy.management.CreateFileGDB(P(D), 'w.gdb')
arcpy.management.CopyFeatures(bs, gdb + '/big')
arcpy.management.AddField(gdb + '/big', 'BID', 'BIGINTEGER')
arcpy.management.CalculateField(gdb + '/big', 'BID', '!UID! * 10000000000', 'PYTHON3')
go('g_bid', gdb + '/big', 'K_NEAREST_NEIGHBORS', Number_of_Neighbors=3, uid='BID')
go('g_oid', gdb + '/big', 'K_NEAREST_NEIGHBORS', Number_of_Neighbors=3, uid='OBJECTID')
go('g_uid', gdb + '/big', 'K_NEAREST_NEIGHBORS', Number_of_Neighbors=3, uid='UID')
json.dump(out, open(P(D) + '/out.json', 'w'))
