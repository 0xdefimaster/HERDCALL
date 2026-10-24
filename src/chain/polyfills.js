// web3.js and anchor use Node's Buffer. Imported first in main.jsx so it exists before they load.
import { Buffer } from 'buffer';

if (!globalThis.Buffer) globalThis.Buffer = Buffer;
