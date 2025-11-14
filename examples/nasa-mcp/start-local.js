#!/usr/bin/env node
import dotenv from 'dotenv';
import { spawn } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

dotenv.config();

const NASA_MCP_PORT = process.env.NASA_MCP_PORT || '3000';
const PCTX_PORT = process.env.PCTX_PORT || '8080';
const USE_LOCAL = process.argv.includes('--local');

console.log('Starting NASA MCP server on port', NASA_MCP_PORT);
const nasaMcp = spawn('node', [
  path.resolve(__dirname, 'nasa-mcp-server.js')
], {
  env: { ...process.env, NASA_MCP_PORT },
  stdio: 'inherit'
});

// Wait for NASA MCP server to start
setTimeout(() => {
  let pctxCommand, pctxArgs, pctxOptions;

  if (USE_LOCAL) {
    console.log('\nStarting pctx (via cargo run) on port', PCTX_PORT);
    const projectRoot = path.resolve(__dirname, '../..');
    pctxCommand = 'cargo';
    pctxArgs = [
      'run',
      '--bin', 'pctx',
      '--',
      'start',
      '--port', PCTX_PORT,
      '--config', path.resolve(__dirname, 'pctx.json')
    ];
    pctxOptions = {
      stdio: 'inherit',
      env: process.env,
      cwd: projectRoot
    };
  } else {
    console.log('\nStarting pctx on port', PCTX_PORT);
    pctxCommand = 'pctx';
    pctxArgs = [
      'start',
      '--port', PCTX_PORT,
      '--config', 'pctx.json'
    ];
    pctxOptions = {
      stdio: 'inherit',
      env: process.env
    };
  }

  const pctx = spawn(pctxCommand, pctxArgs, pctxOptions);

  pctx.on('exit', (code) => {
    console.log('pctx exited with code', code);
    nasaMcp.kill();
    process.exit(code);
  });
}, 2000);

nasaMcp.on('exit', (code) => {
  console.log('NASA MCP exited with code', code);
  process.exit(code);
});

process.on('SIGINT', () => {
  console.log('\nShutting down...');
  nasaMcp.kill();
  process.exit(0);
});
