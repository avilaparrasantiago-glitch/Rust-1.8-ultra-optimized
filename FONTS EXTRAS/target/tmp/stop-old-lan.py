import ctypes, os, sys
from ctypes import wintypes
k=ctypes.WinDLL('kernel32',use_last_error=True)
pid=int(sys.argv[1])
k.FreeConsole()
if not k.AttachConsole(pid):
    sys.exit(2)
members=(wintypes.DWORD*32)()
n=k.GetConsoleProcessList(members,32)
if not 1 <= n <=32 or not set(members[:n]) <= {pid,os.getpid()}:
    sys.exit(3)
k.SetConsoleCtrlHandler(None, True)
sys.exit(0 if k.GenerateConsoleCtrlEvent(0,0) else 4)
