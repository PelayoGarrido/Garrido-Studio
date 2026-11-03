from pathlib import Path, PurePosixPath
import ctypes
import hashlib
import io
import json
import re
import tarfile

from dev import ROOT, own_directory, generated


def payload(data):
    if data[:8] != b"!<arch>\n":
        raise ValueError("Paquete Debian sin cabecera ar")
    at = 8
    for _ in range(32):
        header = data[at:at + 60]
        if len(header) != 60 or header[58:] != b"`\n":
            raise ValueError("Paquete Debian truncado")
        name = header[:16].decode("ascii").strip().rstrip("/")
        size = int(header[48:58].decode("ascii"))
        start = at + 60
        if size < 0 or start + size > len(data):
            raise ValueError("Miembro ar fuera de límites")
        if name == "data.tar.zst":
            return data[start:start + size]
        if name == "data.tar.xz":
            import lzma
            decoder = lzma.LZMADecompressor()
            result = decoder.decompress(data[start:start + size], max_length=32 * 1024**2 + 1)
            if len(result) > 32 * 1024**2 or not decoder.eof or decoder.unused_data:
                raise ValueError("Archivo xz fuera de límites")
            return result
        at = start + size + (size % 2)
    raise ValueError("Falta data.tar en el paquete")


def decompress(data):
    if data[:4] != b"\x28\xb5\x2f\xfd":
        return data
    library = ctypes.CDLL("/opt/homebrew/lib/libzstd.dylib")
    library.ZSTD_getFrameContentSize.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    library.ZSTD_getFrameContentSize.restype = ctypes.c_ulonglong
    library.ZSTD_decompress.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t]
    library.ZSTD_decompress.restype = ctypes.c_size_t
    library.ZSTD_isError.argtypes = [ctypes.c_size_t]
    library.ZSTD_isError.restype = ctypes.c_uint
    source = ctypes.create_string_buffer(data)
    size = library.ZSTD_getFrameContentSize(source, len(data))
    if size == 0xFFFFFFFFFFFFFFFF:
        size = 32 * 1024**2
    if size > 32 * 1024**2:
        raise ValueError("Archivo de desarrollo demasiado grande")
    target = ctypes.create_string_buffer(size)
    actual = library.ZSTD_decompress(target, size, source, len(data))
    if library.ZSTD_isError(actual) or actual > size:
        raise ValueError("Archivo zstd incompleto")
    return ctypes.string_at(target, actual)


def main():
    sysroot = ROOT / ".toolchain/linux-sysroot"
    own_directory(sysroot)
    records = []
    packages = json.loads((ROOT / "outputs/linux-sysroot-packages.json").read_text())
    for package in packages:
        archive = ROOT / "outputs/linux-debs" / package["url"].rsplit("/", 1)[1]
        data = archive.read_bytes()
        if len(data) != package["size"] or hashlib.sha256(data).hexdigest() != package["sha256"]:
            raise ValueError("El paquete no coincide con el índice oficial")
        body = decompress(payload(data))
        if len(body) > 32 * 1024**2:
            raise ValueError("Paquete descomprimido fuera de límites")
        with tarfile.open(fileobj=io.BytesIO(body)) as tar:
            for member in tar.getmembers():
                path = PurePosixPath(member.name.removeprefix("./"))
                if path.is_absolute() or ".." in path.parts:
                    raise ValueError("Ruta fuera del sysroot")
                name = str(path)
                if not member.isfile() or not name.startswith(("usr/include/", "usr/lib/x86_64-linux-gnu/", "lib/x86_64-linux-gnu/")):
                    continue
                if not (name.endswith((".h", ".pc")) or re.search(r"\.so(?:\.\d+)*$", name)):
                    continue
                if member.size > 8 * 1024**2:
                    raise ValueError("Biblioteca demasiado grande")
                with tar.extractfile(member) as source:
                    content = source.read()
                target = sysroot.joinpath(*path.parts)
                own_directory(target.parent)
                if target.exists() and target.read_bytes() == content:
                    continue
                generated(target, content)
                records.append({"path": name, "sha256": hashlib.sha256(content).hexdigest(), "package": package["name"]})
    lib = sysroot / "usr/lib/x86_64-linux-gnu"
    own_directory(lib)
    for name in ["asound", "gcc_s", "ffi", "wayland-client", "wayland-cursor", "wayland-egl", "xkbcommon"]:
        files = [p for base in (lib, sysroot / "lib/x86_64-linux-gnu") for p in base.glob("lib" + name + ".so.*") if p.is_file()]
        if not files:
            raise ValueError("Falta una biblioteca del sysroot: " + name)
        source = max(files, key=lambda p: len(p.name))
        generated(lib / ("lib" + name + ".so"), source.read_bytes())
    generated(ROOT / "outputs/linux-sysroot-provenance.json", json.dumps({"packages": packages, "files": records, "installed_in_macos": False}, indent=2) + "\n")
    print("Sysroot preparado con bibliotecas reales; no se ejecutan scripts Debian.")


if __name__ == "__main__":
    main()
