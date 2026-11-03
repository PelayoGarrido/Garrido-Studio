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

    def number(at, fmt):
        return struct.unpack_from(fmt, data, at)[0]

    if len(data) < 256 or data[:2] != b"MZ":
        raise ValueError("Falta la cabecera DOS")
    pe = number(60, "<I")
    if pe > len(data) - 256 or data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("Cabecera PE truncada")
    machine, count = struct.unpack_from("<HH", data, pe + 4)
    optional = pe + 24
    size = number(pe + 20, "<H")
    if machine != 0x8664 or not 1 <= count <= 96 or size < 128 or number(optional, "<H") != 0x20B:
        raise ValueError("Se requiere PE32+ AMD64")
    sections_at = optional + size
    if sections_at + count * 40 > len(data):
        raise ValueError("Tabla de secciones truncada")
    sections = []
    for i in range(count):
        offset = sections_at + i * 40
        virtual, raw_size, raw = struct.unpack_from("<III", data, offset + 12)
        if raw + raw_size > len(data):
            raise ValueError("Sección fuera del ejecutable")
        sections.append((virtual, raw_size, raw))

    def address(rva, length):
        candidates = [raw + rva - start for start, size, raw in sections if start <= rva and rva + length <= start + size]
        if len(candidates) != 1:
            raise ValueError("Dirección PE fuera de una sección única")
        return candidates[0]

    imports = []
    rva, length = struct.unpack_from("<II", data, optional + 120)
    if rva:
        for index in range(min(length // 20, 256)):
            offset = address(rva + index * 20, 20)
            descriptor = data[offset:offset + 20]
            if descriptor == bytes(20):
                break
            name = address(number(offset + 12, "<I"), 1)
            end = data.find(b"\0", name, min(name + 261, len(data)))
            if end < 0:
                raise ValueError("Nombre de biblioteca truncado")
            library = data[name:end].decode("ascii")
            if not library or any(c in library for c in "/\\:"):
                raise ValueError("Nombre de biblioteca no válido")
            imports.append(library)
        else:
            raise ValueError("Directorio de importación sin terminador")
    flags = number(optional + 70, "<H")
    if flags & 0x140 != 0x140:
        raise ValueError("Faltan las protecciones ASLR/NX")
    subsystem = number(optional + 68, "<H")
    return {"file": path.name, "machine": "AMD64", "format": "PE32+", "bytes": len(data),
            "subsystem": {2: "windows", 3: "console"}.get(subsystem, str(subsystem)),
            "sha256": hashlib.sha256(data).hexdigest(), "aslr": True, "nx": True,
            "high_entropy_va": bool(flags & 0x20), "imports": sorted(set(imports)), "executed": False}


if __name__ == "__main__":
    print(json.dumps([inspect(name) for name in sys.argv[1:]], indent=2))
