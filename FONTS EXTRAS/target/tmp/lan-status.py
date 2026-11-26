import json
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
def readvar(f):
    value = 0
    for i in range(5):
        b = f.read(1)
        if not b:
            raise EOFError()
        value |= (b[0] & 127) << (7*i)
        if b[0] < 128:
            return value
    raise ValueError('invalid status response')

try:
    with socket.create_connection((host, port), 2) as sock:
        address = host.encode('ascii')
        hello = bytes([0,47,len(address)]) + address + port.to_bytes(2,'big') + bytes([1])
        sock.sendall(bytes([len(hello)]) + hello + bytes([1,0]))
        with sock.makefile('rb') as f:
            readvar(f)
            assert readvar(f) == 0
            data=json.loads(f.read(readvar(f)))
            assert data['version']['protocol']==47
            if len(sys.argv)>3:
                assert sys.argv[3] in str(data['description'])
            print(json.dumps(data,ensure_ascii=True))
except (OSError,EOFError,ValueError,AssertionError):
    sys.exit(1)
