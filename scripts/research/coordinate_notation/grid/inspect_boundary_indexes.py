"""Compare independent SBN layouts: headers, record boxes and node placement."""
import json
import struct
import subprocess
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[4]


def parse(path):
    data=path.read_bytes();sbx=path.with_suffix('.sbx').read_bytes()
    assert struct.unpack_from('>I',data,24)[0]*2==len(data)
    assert struct.unpack_from('>I',sbx,24)[0]*2==len(sbx)
    blocks={};offset=100
    while offset<len(data):
        number,words=struct.unpack_from('>II',data,offset)
        xoffset,xwords=struct.unpack_from('>II',sbx,100+(number-1)*8)
        assert xoffset*2==offset and words==xwords
        blocks[number]=data[offset+8:offset+8+words*2]
        offset+=8+words*2
    assert offset==len(data) and len(sbx)==100+8*len(blocks)
    entries={}
    for i in range(0,len(blocks[1]),8):
        node=i//8+1;first,count=struct.unpack_from('>ii',blocks[1],i)
        if count==0:
            assert first==-1;continue
        seen=0;binno=first
        while seen<count:
            block=blocks[binno];assert len(block)%8==0 and len(block)<=800
            for p in range(0,len(block),8):
                box=list(block[p:p+4]);record=struct.unpack_from('>I',block,p+4)[0]
                assert record not in entries
                entries[record]={'box':box,'node':node};seen+=1
                lo=[0,0];hi=[256,256]
                for depth,bit in enumerate(bin(node)[3:]):
                    axis=depth%2;mid=(lo[axis]+hi[axis])//2
                    if bit=='0': assert box[axis]>mid;(lo.__setitem__(axis,mid))
                    else: assert box[axis+2]<=mid;(hi.__setitem__(axis,mid))
            binno+=1
        assert seen==count
    return data,entries


def main():
    results = []
    with tempfile.TemporaryDirectory(prefix="unigis-gars-index-") as directory:
        for fmt in ["gars", "georef"]:
            original = (ROOT / "testdata/arcgis-reference" /
                        f"ccn_grid_input_boundary_{fmt}_gars.sbn")
            native = Path(directory) / (fmt + ".sbn")
            subprocess.run(
                [str(ROOT / "target/debug/ug"), "copy-features",
                 str(original.with_suffix(".shp")), str(native.with_suffix(".shp"))],
                check=True, capture_output=True, text=True,
            )
            a, ea = parse(original)
            b, eb = parse(native)
            header_equal = a[:24] + a[28:100] == b[:24] + b[28:100]
            missing = sorted(set(ea) ^ set(eb))
            box_changes = [i for i in ea.keys() & eb.keys()
                           if ea[i]["box"] != eb[i]["box"]]
            node_changes = [
                {"record": i, "box": ea[i]["box"],
                 "arcgis_node": ea[i]["node"], "native_node": eb[i]["node"]}
                for i in sorted(ea.keys() & eb.keys())
                if ea[i]["node"] != eb[i]["node"]
            ]
            results.append({"format": fmt, "header_equal": header_equal,
                            "records": len(ea), "missing": missing,
                            "box_changes": box_changes, "node_changes": node_changes})
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
