exec(open('/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/p2.py').read().split("roads = P")[0])
R='/mnt/media/ArcGIS_Linux/testdata/'
chain = P(R+'dm/chain.shp'); wells=P(G+'wells.shp')
go('unsplit', lambda o: M.UnsplitLine(chain, o), 20)
go('unsplit_kind', lambda o: M.UnsplitLine(chain, o, 'KIND'), 20)
go('unsplit_cls', lambda o: M.UnsplitLine(chain, o, 'KIND;CLASS'), 20)
go('unsplit_stats', lambda o: M.UnsplitLine(chain, o, 'KIND', [['LINE_ID','SUM'],['CLASS','MAX'],['LINE_ID','COUNT']]), 20)
go('unsplit_conc', lambda o: M.UnsplitLine(chain, o, 'KIND', [['KIND','CONCATENATE']], '|'), 20)
go('slv', lambda o: M.SplitLine(chain, o), 40)
