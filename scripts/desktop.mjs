import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const args = process.argv.slice(2);
const localWindowsTools = process.platform === 'win32' && existsSync(new URL('../.tools/cargo/bin/cargo.exe', import.meta.url));
const command = localWindowsTools ? 'powershell.exe' : process.execPath;
const commandArgs = localWindowsTools
  ? ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', fileURLToPath(new URL('./desktop.ps1', import.meta.url)), ...args]
  : [fileURLToPath(new URL('../node_modules/@tauri-apps/cli/tauri.js', import.meta.url)), ...args];
const child = spawn(command, commandArgs, { cwd: root, stdio: 'inherit', shell: false });
child.on('error', error => { console.error(error.message); process.exitCode = 1; });
child.on('exit', code => { process.exitCode = code ?? 1; });

