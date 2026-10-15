#!/usr/bin/env python3
"""Read a Blitz3D PE executable and extract its relocatable x86 game module.

Never modifies the input executable. The linked analysis image uses synthetic
addresses for external runtime calls and is ONLY for static disassembly.

Format reference (original engine author):
https://github.com/blitz-research/blitz3d/blob/master/bbruntime_dll/bbruntime_dll.cpp
https://github.com/blitz-research/blitz3d/blob/master/linker/linker.cpp
"""

import argparse
import bisect
import hashlib
import json
from pathlib import Path
import struct
import subprocess


def resources(data):
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    assert data[pe:pe + 4] == b'PE\0\0'
    count = struct.unpack_from('<H', data, pe + 6)[0]
    optional_size = struct.unpack_from('<H', data, pe + 20)[0]
    assert struct.unpack_from('<H', data, pe + 24)[0] == 0x10b, 'Requires PE32'
    section_table = pe + 24 + optional_size
    sections = []
    for i in range(count):
        virtual_size, rva, size, offset = struct.unpack_from(
            '<IIII', data, section_table + 40 * i + 8)
        sections.append((rva, max(size, virtual_size), offset))

    def file_offset(rva):
        for start, size, offset in sections:
            if start <= rva < start + size:
                return offset + rva - start
        raise ValueError(f'Unmapped RVA {rva:#x}')

    root = file_offset(struct.unpack_from('<I', data, pe + 24 + 112)[0])

    def walk(offset=0, path=()):
        named, ids = struct.unpack_from('<HH', data, root + offset + 12)
        for i in range(named + ids):
            name, value = struct.unpack_from('<II', data, root + offset + 16 + i * 8)
            if name & 0x80000000:
                pos = root + (name & 0x7fffffff)
                size = struct.unpack_from('<H', data, pos)[0]
                name = data[pos + 2:pos + 2 + size * 2].decode('utf-16le')
            if value & 0x80000000:
                yield from walk(value & 0x7fffffff, path + (name,))
            else:
                rva, size = struct.unpack_from('<II', data, root + value)
                yield path + (name,), (file_offset(rva), size)

    return dict(walk())


def extract_module(data):
    entries = resources(data)
    offset, size = next(v for k, v in entries.items() if k[:2] == (10, 1111))
    length = struct.unpack_from('<I', data, offset)[0]
    begin = offset + 4
    pos = begin + length
    result = {'file_offset': begin, 'module_length': length}
    for table in ('symbols', 'relative', 'absolute'):
        count = struct.unpack_from('<I', data, pos)[0]
        pos += 4
        values = []
        for _ in range(count):
            end = data.index(0, pos)
            name = data[pos:end].decode('ascii')
            value = struct.unpack_from('<I', data, end + 1)[0]
            pos = end + 5
            if value >= length:
                raise ValueError(f'{name}: module offset outside module')
            values.append([name, value])
        result[table] = values
    assert pos == offset + size, 'Resource contains unexpected trailing bytes'
    return data[begin:begin + length], result


def link_for_analysis(module, tables):
    data = bytearray(module)
    symbols = dict(tables['symbols'])
    external = sorted({s for s, _ in tables['relative'] + tables['absolute'] if s not in symbols})
    # Outside module, enough room for an unambiguous fake address for each call.
    external_base = (len(module) + 0xfffff) & ~0xfffff
    symbols.update({s: external_base + i * 16 for i, s in enumerate(external)})
    for kind in ('relative', 'absolute'):
        for name, pos in tables[kind]:
            value = struct.unpack_from('<I', data, pos)[0]
            value += symbols[name] - (pos if kind == 'relative' else 0)
            struct.pack_into('<I', data, pos, value & 0xffffffff)
    return data, symbols


def annotated_disassembly(path, tables, symbols, start, end):
    result = subprocess.run([
        'objdump', '-D', '-b', 'binary', '-m', 'i386', '-M', 'intel',
        '--insn-width=16', f'--start-address={start}', f'--stop-address={end}', str(path)
    ], text=True, capture_output=True, check=True)
    reverse = {value: name for name, value in symbols.items()}
    relocations = {value: name for name, value in tables['relative'] + tables['absolute']}
    positions = sorted(relocations)
    lines = []
    for line in result.stdout.splitlines():
        try:
            pc = int(line.split(':')[0], 16)
        except ValueError:
            continue
        if pc in reverse:
            lines.append(reverse[pc] + ':')
        nbytes = len(line.split('\t')[1].split())
        first = bisect.bisect_left(positions, pc)
        last = bisect.bisect_left(positions, pc + nbytes)
        refs = [relocations[p] for p in positions[first:last]]
        lines.append(line + (' ; ' + ', '.join(refs) if refs else ''))
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--output', type=Path, default=Path('.work/analysis'))
    parser.add_argument('--disassemble', nargs=2, metavar=('START', 'END'),
                        help='Module offsets (0x allowed) or named symbols')
    args = parser.parse_args()
    data = args.executable.read_bytes()
    module, tables = extract_module(data)
    tables['source_sha256'] = hashlib.sha256(data).hexdigest()
    linked, symbols = link_for_analysis(module, tables)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / 'module.bin').write_bytes(module)
    (args.output / 'module-symbols.json').write_text(json.dumps(tables) + '\n')
    linked_path = args.output / 'module-linked-analysis.bin'
    linked_path.write_bytes(linked)
    if args.disassemble:
        offsets = [symbols[s] if s in symbols else int(s, 0) for s in args.disassemble]
        print(annotated_disassembly(linked_path, tables, symbols, *offsets), end='')
    else:
        print(json.dumps({'source': str(args.executable), 'sha256': tables['source_sha256'],
                          'module_file_offset': hex(tables['file_offset']),
                          'module_length': len(module),
                          **{key: len(tables[key]) for key in ('symbols', 'relative', 'absolute')}}, indent=2))


if __name__ == '__main__':
    main()
