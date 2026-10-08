"""打 Windows 分发包：dist/MTerm-v<版本>-win-x64.zip

内容 = 单文件 exe + README.md + CHANGELOG.md。
打包前先用 scripts/pe-deps.py 校验 exe 是否自足 —— mingw 工具链（windows-gnu）
若动态依赖 libgcc_s_seh-1.dll / libwinpthread-1.dll，必须随包附带，否则目标机起不来。

用法:  python scripts/package-win.py            # 用已构建的 release exe
       python scripts/package-win.py --build    # 先 cargo build --release 再打包
"""
import os
import re
import shutil
import subprocess
import sys
import zipfile

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(ROOT, "target", "release", "MTerm.exe")
DIST = os.path.join(ROOT, "dist")
EXTRA = ["README.md", "CHANGELOG.md"]


def cargo_version():
    src = open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8").read()
    m = re.search(r'(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"', src)
    if not m:
        raise SystemExit("!! 无法从 Cargo.toml 读取 [package].version")
    return m.group(1)


def main():
    if "--build" in sys.argv:
        print("[1/4] cargo build --release ...")
        rc = subprocess.call(["cargo", "build", "--release"], cwd=ROOT)
        if rc != 0:
            raise SystemExit("!! 构建失败，已中止打包")
    else:
        print("[1/4] 跳过构建（如需先构建，加 --build）")

    if not os.path.isfile(EXE):
        raise SystemExit("!! 找不到 %s —— 先跑 cargo build --release" % EXE)

    ver = cargo_version()
    print("[2/4] 校验 exe 依赖 ...")
    subprocess.call([sys.executable, os.path.join(ROOT, "scripts", "pe-deps.py"), EXE])

    name = "MTerm-v%s-win-x64" % ver
    stage = os.path.join(DIST, name)
    if os.path.isdir(stage):
        shutil.rmtree(stage)
    os.makedirs(stage)

    print("[3/4] 组装 %s ..." % os.path.relpath(stage, ROOT))
    shutil.copy2(EXE, os.path.join(stage, "MTerm.exe"))
    for f in EXTRA:
        p = os.path.join(ROOT, f)
        if os.path.isfile(p):
            shutil.copy2(p, os.path.join(stage, f))
        else:
            print("      (跳过缺失的 %s)" % f)

    zip_path = os.path.join(DIST, name + ".zip")
    if os.path.isfile(zip_path):
        os.remove(zip_path)
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as z:
        for f in sorted(os.listdir(stage)):
            z.write(os.path.join(stage, f), os.path.join(name, f))

    print("[4/4] 完成")
    print("    exe : %s (%.1f MB)"
          % (os.path.relpath(EXE, ROOT), os.path.getsize(EXE) / 1048576))
    print("    zip : %s (%.1f MB)"
          % (zip_path, os.path.getsize(zip_path) / 1048576))
    with zipfile.ZipFile(zip_path) as z:
        for i in z.infolist():
            print("          %-40s %8.2f MB" % (i.filename, i.file_size / 1048576))
    return 0


if __name__ == "__main__":
    sys.exit(main())
