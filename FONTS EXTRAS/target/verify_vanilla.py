import os, subprocess, json, sys, time, hashlib, urllib.request
from pathlib import Path
version = sys.argv[1]
mc = Path(os.environ["APPDATA"]) / ".minecraft"
work = Path(__file__).resolve().parent / ("vanilla-" + version)
work.mkdir(exist_ok=True)
def fetch(url):
    from urllib.parse import urlparse
    assert urlparse(url).scheme == "https" and urlparse(url).hostname in {
        "piston-meta.mojang.com","piston-data.mojang.com","libraries.minecraft.net",
        "launchermeta.mojang.com","launcher.mojang.com"}
    with urllib.request.urlopen(url,timeout=20) as f:
        return f.read()
def checked(meta,path):
    if not path.is_file():
        data=fetch(meta["url"])
        assert hashlib.sha1(data).hexdigest()==meta["sha1"]
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(data)
    return path
folder = mc / "versions" / version
if folder.is_dir():
    spec=json.loads((folder/(version+".json")).read_bytes())
    jar=folder/(version+".jar")
else:
    manifest=json.loads(fetch("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"))
    entry=next(v for v in manifest["versions"] if v["id"]==version)
    spec=json.loads(checked(entry,work/"version.json").read_bytes())
    jar=checked(spec["downloads"]["client"],work/"client.jar")
classpath=[]
for lib in spec["libraries"]:
    rules=lib.get("rules")
    if rules:
        allow=False
        for rule in rules:
            if rule.get("os",{}).get("name","windows")=="windows":
                allow=rule["action"]=="allow"
        if not allow:continue
    art=lib.get("downloads",{}).get("artifact")
    if art:
        rel=art.get("path") or art["url"].split("libraries.minecraft.net/")[1]
        local=mc/"libraries"/rel
        classpath.append(local if local.is_file() else checked(art,work/"libraries"/rel))
classpath.append(jar)
natives=next((mc/"versions/1.8.9").glob("*-natives-*"))
jvm=next((mc/"runtime/jre-legacy").rglob("javaw.exe"))
args=[str(jvm),"-Xms64m","-Xmx384m","-Djava.library.path="+str(natives),"-cp",os.pathsep.join(map(str,classpath)),
      spec["mainClass"],"--username","TestJava"+version.replace(".",""),"--version",version,"--gameDir",str(work),
      "--assetsDir",str(mc/"assets"),"--assetIndex",spec["assets"],"--uuid","00000000000040008000000000000047",
      "--accessToken","0","--userProperties","{}","--userType","legacy","--server","192.168.1.150","--port","25567",
      "--width","640","--height","360"]
startup=subprocess.STARTUPINFO()
startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
with (work/"client-output.log").open("w") as log:
    p=subprocess.Popen(args,stdout=log,stderr=log,cwd=work,startupinfo=startup)
    try:
        for i in range(75):
            if p.poll() is not None: raise RuntimeError("client exited: "+str(p.returncode))
            time.sleep(1)
            with urllib.request.urlopen("http://127.0.0.1:9901/metrics",timeout=3) as r:
                metrics=r.read().decode().splitlines()
            if any(line.startswith("coral_players ") and int(line.split()[1])>=1 for line in metrics):
                time.sleep(5)
                if p.poll() is not None: raise RuntimeError("client exited after login")
                log_path=work/"logs/latest.log"
                if log_path.exists():
                    print("\n".join(log_path.read_text(errors="replace").splitlines()[-6:]))
                print("Minecraft",version,"joined, pid",p.pid)
                break
        else: raise RuntimeError("login timeout")
    finally:
        if p.poll() is None:p.terminate()
        p.wait(timeout=10)