#!/usr/bin/env python

import argparse

def bytearray_to_le16(out, data):
    # decode and encode both as little endian
    shorts = [(ord(data[i]) | ord(data[i+1]) << 8) for i in range(0, len(data), 2)]
    for line in [shorts[i:i+8] for i in range(0, len(shorts), 8)]:
        out.write('    ');
        out.write(', '.join(['0x{short:04x}'.format(short=x) for x in line]));
        out.write(',\n');

def main():
    cmdline = argparse.ArgumentParser()
    cmdline.add_argument('-i', '--infile', required=True)
    cmdline.add_argument('-o', '--outfile', required=True)

    args = cmdline.parse_args()
    with open(args.infile, 'rb') as f:
        data = f.read()

    with open(args.outfile, 'w') as f:
        f.write("pub const TEST_DATA: &[u16] = &[\n")
        bytearray_to_le16(f, data)
        f.write("];\n");

if __name__ == '__main__':
    main()
