#!/usr/bin/env node
require('dotenv').config();
const { spawn } = require('child_process');
const readline = require('readline');
const path = require('path');

const NOTION_MCP_PORT = process.env.NOTION_MCP_PORT || '3000';
const PCTX_PORT = process.env.PCTX_PORT || '8080';
const USE_LOCAL = process.argv.includes('--local');

console.log('Starting Notion MCP server on port', NOTION_MCP_PORT);
const notionMcp = spawn('npx', [
  '-y',
  '@notionhq/notion-mcp-server',
  '--transport', 'http',
  '--port', NOTION_MCP_PORT
], {
  env: process.env
});

let authTokenCaptured = false;

// Capture the auth token from Notion MCP output
const rl = readline.createInterface({
  input: notionMcp.stdout,
  crlfDelay: Infinity
});

rl.on('line', (line) => {
  console.log('[Notion MCP]', line);

  if (!authTokenCaptured && line.includes('Generated auth token:')) {
    const token = line.split('Generated auth token: ')[1];
    if (token) {
      console.log('\nCaptured auth token from Notion MCP');
      process.env.NOTION_MCP_AUTH_TOKEN = token;
      authTokenCaptured = true;

      // Start pctx now that we have the token
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
            '--config', path.resolve(__dirname, 'pctx-notion.json')
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
            '--config', 'pctx-notion.json'
          ];
          pctxOptions = {
            stdio: 'inherit',
            env: process.env
          };
        }

        const pctx = spawn(pctxCommand, pctxArgs, pctxOptions);

        pctx.on('exit', (code) => {
          console.log('pctx exited with code', code);
          notionMcp.kill();
          process.exit(code);
        });
      }, 2000);
    }
  }
});

notionMcp.stderr.on('data', (data) => {
  console.error('[Notion MCP Error]', data.toString());
});

notionMcp.on('exit', (code) => {
  console.log('Notion MCP exited with code', code);
  process.exit(code);
});

process.on('SIGINT', () => {
  console.log('\nShutting down...');
  notionMcp.kill();
  process.exit(0);
});
