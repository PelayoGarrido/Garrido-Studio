from pathlib import Path
import hashlib
import json
import struct
import sys


def inspect(path):
    path = Path(path)
    if path.is_symlink() or path.stat().st_size > 64 * 1024**2:
        raise ValueError("Ejecutable fuera de límites")
    data = path.read_bytes()
    if len(data) < 64 or data[:7] != b"\x7fELF\x02\x01\x01":
        raise ValueError("Se requiere ELF64 little endian")
    kind, machine = struct.unpack_from("<HH", data, 16)
    if machine != 62 or kind not in (2, 3):
        raise ValueError("Se requiere ELF AMD64 ejecutable o compartido")
    table = struct.unpack_from("<Q", data, 32)[0]
    entry, count = struct.unpack_from("<HH", data, 54)
    if entry < 56 or count > 128 or table + entry * count > len(data):
        raise ValueError("Tabla de segmentos truncada")
    segments = [struct.unpack_from("<IIQQQQQQ", data, table + i * entry) for i in range(count)]
    for _, _, offset, _, _, size, _, _ in segments:
        if offset + size > len(data):
            raise ValueError("Segmento fuera del ejecutable")

    def address(rva, size):
        found = [offset + rva - virtual for typ, _, offset, virtual, _, length, _, _ in segments
                 if typ == 1 and virtual <= rva and rva + size <= virtual + length]
        if len(found) != 1:
            raise ValueError("Dirección ELF fuera de un segmento único")
        return found[0]

    tags = {}
    for typ, _, offset, _, _, length, _, _ in segments:
        if typ != 2:
            continue
        if length > 65536 or length % 16:
            raise ValueError("Sección dinámica fuera de límites")
        for at in range(offset, offset + length, 16):
            tag, value = struct.unpack_from("<qQ", data, at)
            if tag == 0:
                break
            tags.setdefault(tag, []).append(value)
    strings = tags.get(5, [0])[0]
    string_size = tags.get(10, [0])[0]

    def text(offset):
        if offset >= string_size:
            raise ValueError("Texto ELF fuera de límites")
        start = address(strings + offset, 1)
        end = data.find(b"\0", start, min(start + string_size - offset, start + 4096))
        if end < 0:
            raise ValueError("Texto ELF sin terminador")
        return data[start:end].decode("ascii")

    needed = [text(offset) for offset in tags.get(1, [])]
    paths = [text(offset) for tag in (15, 29) for offset in tags.get(tag, [])]
    if any("/Users/" in value or ".toolchain" in value for value in needed + paths):
        raise ValueError("Ruta de compilación filtrada al cargador")
    stack = [flags for typ, flags, *_ in segments if typ == 0x6474E551]
    nx = bool(stack) and all(flags & 1 == 0 for flags in stack)
    relro = any(typ == 0x6474E552 for typ, *_ in segments)
    now = 24 in tags or any(value & 8 for value in tags.get(30, [])) or any(value & 1 for value in tags.get(0x6FFFFFFB, []))
    if not nx or not relro or not now:
        raise ValueError("Faltan NX o RELRO completo")
    versions = []
    cursor = tags.get(0x6FFFFFFE, [0])[0]
    for _ in range(min(tags.get(0x6FFFFFFF, [0])[0], 128)):
        at = address(cursor, 16)
        _, count, _, auxiliary, following = struct.unpack_from("<HHIII", data, at)
        if count > 256:
            raise ValueError("Demasiadas versiones ELF")
        item = cursor + auxiliary
        for _ in range(count):
            at = address(item, 16)
            _, _, _, name, next_item = struct.unpack_from("<IHHII", data, at)
            versions.append(text(name))
            item += next_item
        if following == 0:
            break
        cursor += following
    glibc = sorted({v for v in versions if v.startswith("GLIBC_")})
    return {"file": path.name, "machine": "AMD64", "format": "ELF64", "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(), "nx": nx, "full_relro": bool(relro and now),
            "pie": kind == 3 and any(typ == 3 for typ, *_ in segments), "needed": sorted(needed),
            "runtime_paths": paths, "glibc_versions": glibc, "executed": False}


if __name__ == "__main__":
    print(json.dumps([inspect(name) for name in sys.argv[1:]], indent=2))
