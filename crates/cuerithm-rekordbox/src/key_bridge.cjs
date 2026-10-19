'use strict';
// Owned bridge: inspect only the installed agent's local database-key helper.
// No shipped source, key or account data is bundled in this file.
const fs = process.versions.electron ? require('original-fs') : require('fs');
const vm = require('vm');
const v8 = require('v8');
const inspector = require('inspector');
const nodePath = require('path');
const MAX_KEY = 16384;
const MAX_INPUT = 32768;
let archive;
let session;
function fail() { process.exit(1); }
process.on('uncaughtException', fail);
process.on('unhandledRejection', fail);

function readAt(fd, length, position) {
  const bytes = Buffer.alloc(length);
  let done = 0;
  while (done < length) {
    const n = fs.readSync(fd, bytes, done, length - done, position + done);
    if (!n) throw Error('Truncated artifact');
    done += n;
  }
  return bytes;
}
function openArchive(filename) {
  const fd = fs.openSync(filename, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0));
  archive = fd;
  const stat = fs.fstatSync(fd);
  if (!stat.isFile() || stat.size < 16 || stat.size > 1024 * 1024 * 1024) throw Error('Artifact size');
  const prefix = readAt(fd, 16, 0);
  const headerSize = prefix.readUInt32LE(4);
  const payloadSize = prefix.readUInt32LE(8);
  const jsonSize = prefix.readUInt32LE(12);
  if (prefix.readUInt32LE(0) !== 4 || headerSize < 8 || headerSize > 16 * 1024 * 1024 ||
      payloadSize + 4 !== headerSize || jsonSize > payloadSize - 4 || 8 + headerSize > stat.size) throw Error('Artifact header');
  const header = JSON.parse(readAt(fd, jsonSize, 16).toString('utf8'));
  return function member(name, limit) {
    let entry = header;
    for (const part of name.split('/')) {
      if (!entry.files || !Object.prototype.hasOwnProperty.call(entry.files, part)) throw Error('Missing member');
      entry = entry.files[part];
    }
    if (entry.link || entry.unpacked || !Number.isSafeInteger(entry.size) || entry.size < 1 || entry.size > limit ||
        typeof entry.offset !== 'string' || !/^\d{1,16}$/.test(entry.offset)) throw Error('Unsupported member');
    const start = 8 + headerSize + Number(entry.offset);
    if (!Number.isSafeInteger(start) || start < 8 + headerSize || start + entry.size > stat.size) throw Error('Member bounds');
    return readAt(fd, entry.size, start);
  };
}
function input() {
  const chunks = [];
  let size = 0;
  while (true) {
    const block = Buffer.alloc(4096);
    const n = fs.readSync(0, block, 0, block.length, null);
    if (!n) break;
    size += n;
    if (size > MAX_INPUT) throw Error('Input budget');
    chunks.push(block.subarray(0, n));
  }
  const raw = Buffer.concat(chunks);
  const parsed = JSON.parse(raw.toString('utf8'));
  raw.fill(0);
  for (const chunk of chunks) chunk.fill(0);
  const dp = parsed.dp;
  if (typeof dp !== 'string' || dp.length < 4 || dp.length > MAX_KEY || dp.length % 4 ||
      !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(dp)) throw Error('Invalid selected option');
  const encrypted = Buffer.from(dp, 'base64');
  if (!encrypted.length || encrypted.length % 8) throw Error('Invalid ciphertext');
  return encrypted;
}
function cachedModule(bytes, context) {
  const major = Number(process.versions.node.split('.')[0]);
  if (major < 12 || bytes.length < 24) throw Error('Unsupported runtime');
  v8.setFlagsFromString('--no-lazy');
  const dummy = new vm.Script('"ಠ_ಠ"').createCachedData();
  dummy.copy(bytes, 12, 12, major <= 18 ? 16 : 20);
  const length = bytes.readUInt32LE(8);
  if (length < 2 || length > 4 * 1024 * 1024) throw Error('Invalid cache source length');
  const script = new vm.Script('"' + '\u200b'.repeat(length - 2) + '"', { cachedData: bytes });
  if (script.cachedDataRejected) throw Error('Unsupported bytecode');
  return script.runInContext(context, { timeout: 1500 });
}
function isolatedMain(bytes) {
  const callbacks = new Map();
  let noop;
  noop = new Proxy(function () { return noop; }, {
    get: (_target, key) => key === Symbol.toPrimitive ? () => 0 : key === 'then' ? undefined : noop,
  });
  const stub = values => new Proxy(values, { get: (target, key) => Object.prototype.hasOwnProperty.call(target, key) ? target[key] : noop });
  const app = stub({getPath: () => '/isolated', getVersion: () => '0.0.0', on: (name, fn) => callbacks.set(name, fn), whenReady: () => ({then: () => {}})});
  const path = new Proxy(nodePath, {get: (target, key) => typeof target[key] === 'function' ? (...args) => target[key](...args.map(value => typeof value === 'string' ? value : '/isolated')) : target[key]});
  const modules = {
    path,
    os: {platform: () => process.platform, arch: () => process.arch, homedir: () => '/isolated', release: () => ''},
    electron: stub({app}),
    fs: stub({existsSync: () => false, readFileSync: () => '', readdirSync: () => []}),
    'electron-json-storage': stub({getDataPath: () => '/isolated'}),
  };
  const sandbox = {
    Buffer, module: {exports: {}}, __filename: '/isolated/main_core.jsc', __dirname: '/isolated',
    process: {platform: process.platform, arch: process.arch, versions: {node: process.versions.node}, execPath: '/isolated/agent', resourcesPath: '/isolated', on: () => {}, env: {}, argv: []},
    require: name => Object.prototype.hasOwnProperty.call(modules, name) ? modules[name] : noop,
    console: noop, setTimeout: noop, setInterval: noop, clearTimeout: noop, clearInterval: noop,
  };
  sandbox.exports = sandbox.module.exports;
  sandbox.global = sandbox;
  const context = vm.createContext(sandbox, {codeGeneration: {strings: false, wasm: false}});
  context.wrapper = cachedModule(bytes, context);
  new vm.Script('wrapper(exports,require,module,__filename,__dirname)').runInContext(context, {timeout: 1500});
  // Do not invoke ready or any application lifecycle callback.
  const ready = callbacks.get('ready');
  if (typeof ready !== 'function') throw Error('Unsupported agent entrypoint');
  return ready;
}
function post(method, params = {}) {
  return new Promise((resolve, reject) => session.post(method, params, (err, result) => err ? reject(Error('Runtime inspection failed')) : resolve(result)));
}
async function localSeed(ready) {
  session = new inspector.Session();
  session.connect(); // In-process only: no inspector port/listener.
  global.__cuerithmReady = ready;
  const fn = (await post('Runtime.evaluate', {expression: 'global.__cuerithmReady', silent: true})).result;
  const props = await post('Runtime.getProperties', {objectId: fn.objectId});
  const scopes = props.internalProperties.find(item => item.name === '[[Scopes]]');
  const list = await post('Runtime.getProperties', {objectId: scopes.value.objectId, ownProperties: true});
  const candidates = [];
  for (const scope of list.result.filter(item => /^\d+$/.test(item.name) && item.value.description !== 'Global')) {
    const values = await post('Runtime.getProperties', {objectId: scope.value.objectId, ownProperties: true});
    for (const value of values.result) {
      if (value.name === 'buildBFKey' && value.value && value.value.type === 'function') candidates.push(value.value.objectId);
    }
  }
  if (candidates.length !== 1) throw Error('Unsupported key helper');
  const reply = await post('Runtime.callFunctionOn', {objectId: candidates[0], functionDeclaration: 'function(){return this()}', returnByValue: true, silent: true});
  if (reply.exceptionDetails || reply.result.type !== 'string' || !reply.result.value.length || reply.result.value.length > 56) throw Error('Invalid key helper result');
  session.disconnect(); session = undefined; delete global.__cuerithmReady;
  return reply.result.value;
}
function decoder(bytes) {
  const sandbox = {module: {exports: {}}, Buffer};
  sandbox.exports = sandbox.module.exports;
  const context = vm.createContext(sandbox, {codeGeneration: {strings: false, wasm: false}});
  new vm.Script(bytes.toString('utf8'), {filename: '/isolated/blowfish.js'}).runInContext(context, {timeout: 1500});
  return sandbox.module.exports;
}
(async function () {
  const encrypted = input();
  const member = openArchive(process.argv[1]);
  const directory = process.platform === 'darwin' ? (process.arch === 'arm64' ? 'jsc_arm64' : 'jsc_x64') : 'jsc';
  const ready = isolatedMain(member(directory + '/main_core.jsc', 4 * 1024 * 1024));
  const seed = await localSeed(ready);
  const Blowfish = decoder(member('node_modules/egoroof-blowfish/dist/blowfish.js', 1024 * 1024));
  const bf = new Blowfish(seed, Blowfish.MODE.ECB, Blowfish.PADDING.PKCS5);
  const key = Buffer.from(bf.decode(encrypted, Blowfish.TYPE.UINT8_ARRAY));
  encrypted.fill(0);
  if (!key.length || key.length > MAX_KEY) throw Error('Invalid local key');
  const output = Buffer.alloc(8 + key.length); output.write('CRK1', 0, 'ascii'); output.writeUInt32BE(key.length, 4); key.copy(output, 8); key.fill(0);
  let written = 0;
  while (written < output.length) written += fs.writeSync(1, output, written, output.length - written);
  output.fill(0);
  fs.closeSync(archive); archive = undefined;
  process.exit(0);
})().catch(() => {
  if (session) session.disconnect();
  if (archive !== undefined) fs.closeSync(archive);
  fail();
});
