"""Example 01's calls from Python over gRPC, against `cargo run -p kitchen-server --example serve`.

    pip install grpcio grpcio-tools
    python -m grpc_tools.protoc -I ../../../../contract/proto --python_out=gen --grpc_python_out=gen \
        kitchen/runtime/v1/runtime.proto
    python call_grpc.py gen <port> <token> <folder>

Documents (op descriptions, run records, targets) travel as their JSON inside `bytes` fields.
"""
import json, os, sys, time, grpc
sys.path.insert(0, sys.argv[1])
from kitchen.runtime.v1 import runtime_pb2 as pb, runtime_pb2_grpc as rpc
port, token, folder = sys.argv[2], sys.argv[3], sys.argv[4]
stub = rpc.RuntimeStub(grpc.insecure_channel(f"127.0.0.1:{port}"))
md = [("authorization", f"Bearer {token}")]
caller = pb.Caller(by="whoever", via="python-grpcio")
ops = stub.Call(pb.Request(caller=caller, ops=pb.OpsFilter()), metadata=md)
print("ops:", [json.loads(i)["id"] for i in ops.ops.items])
sig = os.path.join(folder, "sig.bin"); open(sig, "wb").write(b"\x01\x02\x03")
target = json.dumps({"kind": "op", "op": "demo:echo", "params": {}}).encode()
filling = pb.Filling(sources={"signal": pb.Source(path=pb.PathSource(path=sig, type="signal", format="raw"))})
started = stub.Call(pb.Request(caller=caller, run=pb.Run(target=target, fillings=[filling])), metadata=md)
run = started.started.runs[0]
while True:
    a = stub.Call(pb.Request(caller=caller, run_watch=pb.RunWatch(run=run)), metadata=md)
    if a.run.WhichOneof("state") == "ended":
        rec = json.loads(a.run.ended); break
    time.sleep(0.02)
print("run:", rec["status"], rec["by"], rec["via"])
data = stub.Call(pb.Request(caller=caller, result_read=pb.ResultRead(id=rec["outputs"]["copy"])), metadata=md)
print("bytes:", data.bytes)
try:
    stub.Call(pb.Request(caller=caller, types=pb.Types()))
except grpc.RpcError as e:
    print("no token:", e.code())
