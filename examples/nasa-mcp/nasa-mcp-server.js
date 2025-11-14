#!/usr/bin/env node

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StreamableHTTPServerTransport } from '@modelcontextprotocol/sdk/server/streamableHttp.js';
import express from 'express';
import fetch from 'node-fetch';
import dotenv from 'dotenv';
import { z } from 'zod';
import { randomUUID } from 'node:crypto';

dotenv.config();

const NASA_API_KEY = process.env.NASA_API_KEY || 'DEMO_KEY';
const PORT = process.env.NASA_MCP_PORT || 3000;

// Create MCP server
const server = new McpServer({
  name: 'nasa-mcp-server',
  version: '1.0.0'
});

// Tool handlers
async function searchAsteroids({ start_date, end_date }) {
  let url = `https://api.nasa.gov/neo/rest/v1/feed?start_date=${start_date}&api_key=${NASA_API_KEY}`;
  if (end_date) {
    url += `&end_date=${end_date}`;
  }

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`NASA API error: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();

  const result = {
    element_count: data.element_count,
    date_range: {
      start: start_date,
      end: end_date || 'default (7 days)'
    },
    asteroids_by_date: {}
  };

  for (const [date, asteroids] of Object.entries(data.near_earth_objects)) {
    result.asteroids_by_date[date] = asteroids.map(a => ({
      id: a.id,
      name: a.name,
      is_potentially_hazardous: a.is_potentially_hazardous_asteroid,
      estimated_diameter_km: {
        min: a.estimated_diameter.kilometers.estimated_diameter_min,
        max: a.estimated_diameter.kilometers.estimated_diameter_max
      },
      close_approach: a.close_approach_data[0] ? {
        date: a.close_approach_data[0].close_approach_date,
        velocity_kph: a.close_approach_data[0].relative_velocity.kilometers_per_hour,
        miss_distance_km: a.close_approach_data[0].miss_distance.kilometers
      } : null,
      nasa_jpl_url: a.nasa_jpl_url
    }));
  }

  return result;
}

async function lookupAsteroid({ asteroid_id }) {
  const url = `https://api.nasa.gov/neo/rest/v1/neo/${asteroid_id}?api_key=${NASA_API_KEY}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`NASA API error: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();

  return {
    id: data.id,
    name: data.name,
    designation: data.designation,
    is_potentially_hazardous: data.is_potentially_hazardous_asteroid,
    absolute_magnitude: data.absolute_magnitude_h,
    estimated_diameter_km: {
      min: data.estimated_diameter.kilometers.estimated_diameter_min,
      max: data.estimated_diameter.kilometers.estimated_diameter_max
    },
    close_approaches: data.close_approach_data.slice(0, 10).map(ca => ({
      date: ca.close_approach_date,
      date_full: ca.close_approach_date_full,
      velocity_kph: ca.relative_velocity.kilometers_per_hour,
      miss_distance_km: ca.miss_distance.kilometers,
      orbiting_body: ca.orbiting_body
    })),
    total_close_approaches: data.close_approach_data.length,
    nasa_jpl_url: data.nasa_jpl_url
  };
}

async function browseAsteroids({ page = 0, size = 20 }) {
  const url = `https://api.nasa.gov/neo/rest/v1/neo/browse?page=${page}&size=${size}&api_key=${NASA_API_KEY}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`NASA API error: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();

  return {
    page: data.page,
    asteroids: data.near_earth_objects.map(a => ({
      id: a.id,
      name: a.name,
      is_potentially_hazardous: a.is_potentially_hazardous_asteroid,
      absolute_magnitude: a.absolute_magnitude_h,
      estimated_diameter_km: {
        min: a.estimated_diameter.kilometers.estimated_diameter_min,
        max: a.estimated_diameter.kilometers.estimated_diameter_max
      },
      nasa_jpl_url: a.nasa_jpl_url
    }))
  };
}

async function searchSatellites({ search, page = 1, page_size = 20 }) {
  const url = `https://tle.ivanstanojevic.me/api/tle/?search=${encodeURIComponent(search)}&page=${page}&page-size=${page_size}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`TLE API error: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();

  return {
    total_items: data.totalItems,
    page: page,
    page_size: page_size,
    satellites: data.member.map(s => ({
      satellite_id: s.satelliteId,
      name: s.name,
      date: s.date,
      line1: s.line1,
      line2: s.line2
    }))
  };
}

async function lookupSatellite({ satellite_id }) {
  const url = `https://tle.ivanstanojevic.me/api/tle/${satellite_id}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`TLE API error: ${response.status} ${response.statusText}`);
  }

  const data = await response.json();

  return {
    satellite_id: data.satelliteId,
    name: data.name,
    date: data.date,
    line1: data.line1,
    line2: data.line2
  };
}

// Register tools
server.registerTool(
  'search_asteroids',
  {
    title: 'Search Asteroids',
    description: 'Search for Near Earth Objects (asteroids) based on their closest approach date to Earth. Returns detailed information about asteroids including size, velocity, and miss distance.',
    inputSchema: {
      start_date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/).describe('Starting date for asteroid search in YYYY-MM-DD format'),
      end_date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/).optional().describe('Ending date for asteroid search in YYYY-MM-DD format (optional, defaults to 7 days after start_date)')
    },
    outputSchema: {
      element_count: z.number(),
      date_range: z.object({
        start: z.string(),
        end: z.string()
      }),
      asteroids_by_date: z.record(z.array(z.any()))
    }
  },
  async ({ start_date, end_date }) => {
    const output = await searchAsteroids({ start_date, end_date });
    return {
      content: [{ type: 'text', text: JSON.stringify(output, null, 2) }],
      structuredContent: output
    };
  }
);

server.registerTool(
  'lookup_asteroid',
  {
    title: 'Lookup Asteroid',
    description: 'Lookup a specific asteroid based on its NASA JPL small body ID (SPK-ID). Returns comprehensive details including orbital data, close approach history, and physical characteristics.',
    inputSchema: {
      asteroid_id: z.string().describe('NASA JPL small body ID (SPK-ID) of the asteroid')
    },
    outputSchema: {
      id: z.string(),
      name: z.string(),
      is_potentially_hazardous: z.boolean(),
      estimated_diameter_km: z.object({
        min: z.number(),
        max: z.number()
      }),
      close_approaches: z.array(z.any()),
      nasa_jpl_url: z.string()
    }
  },
  async ({ asteroid_id }) => {
    const output = await lookupAsteroid({ asteroid_id });
    return {
      content: [{ type: 'text', text: JSON.stringify(output, null, 2) }],
      structuredContent: output
    };
  }
);

server.registerTool(
  'browse_asteroids',
  {
    title: 'Browse Asteroids',
    description: 'Browse the overall Near Earth Object dataset with pagination support. Returns a list of asteroids with basic information.',
    inputSchema: {
      page: z.number().min(0).optional().describe('Page number for pagination (default: 0)'),
      size: z.number().min(1).max(100).optional().describe('Number of results per page (default: 20)')
    },
    outputSchema: {
      page: z.any(),
      asteroids: z.array(z.any())
    }
  },
  async ({ page, size }) => {
    const output = await browseAsteroids({ page, size });
    return {
      content: [{ type: 'text', text: JSON.stringify(output, null, 2) }],
      structuredContent: output
    };
  }
);

server.registerTool(
  'search_satellites',
  {
    title: 'Search Satellites',
    description: 'Search for satellites by name and retrieve their Two-Line Element (TLE) set records. TLE data includes orbital parameters needed to track satellite positions.',
    inputSchema: {
      search: z.string().describe('Satellite name or partial name to search for (e.g., "ISS", "Hubble")'),
      page: z.number().min(1).optional().describe('Page number for pagination (default: 1)'),
      page_size: z.number().min(1).max(100).optional().describe('Number of results per page (default: 20)')
    },
    outputSchema: {
      total_items: z.number(),
      page: z.number(),
      page_size: z.number(),
      satellites: z.array(z.any())
    }
  },
  async ({ search, page, page_size }) => {
    const output = await searchSatellites({ search, page, page_size });
    return {
      content: [{ type: 'text', text: JSON.stringify(output, null, 2) }],
      structuredContent: output
    };
  }
);

server.registerTool(
  'lookup_satellite',
  {
    title: 'Lookup Satellite',
    description: 'Retrieve TLE data for a specific satellite by its catalog number (NORAD ID). Returns the most recent two-line element set for precise orbital tracking.',
    inputSchema: {
      satellite_id: z.string().describe('Satellite catalog number (NORAD ID), e.g., "25544" for ISS')
    },
    outputSchema: {
      satellite_id: z.number(),
      name: z.string(),
      date: z.string(),
      line1: z.string(),
      line2: z.string()
    }
  },
  async ({ satellite_id }) => {
    const output = await lookupSatellite({ satellite_id });
    return {
      content: [{ type: 'text', text: JSON.stringify(output, null, 2) }],
      structuredContent: output
    };
  }
);

// Setup Express with Streamable HTTP transport
const app = express();
app.use(express.json());

const transports = {};

app.post('/mcp', async (req, res) => {
  const sessionId = req.headers['mcp-session-id'];
  let transport;

  if (sessionId && transports[sessionId]) {
    transport = transports[sessionId];
  } else {
    transport = new StreamableHTTPServerTransport({
      sessionIdGenerator: () => randomUUID(),
      onsessioninitialized: (id) => {
        transports[id] = transport;
        console.log('Session initialized:', id);
      },
      onsessionclosed: (id) => {
        delete transports[id];
        console.log('Session closed:', id);
      }
    });

    transport.onclose = () => {
      if (transport.sessionId) {
        delete transports[transport.sessionId];
      }
    };

    await server.connect(transport);
  }

  await transport.handleRequest(req, res, req.body);
});

app.get('/mcp', async (req, res) => {
  const sessionId = req.headers['mcp-session-id'];
  const transport = transports[sessionId];
  if (transport) {
    await transport.handleRequest(req, res);
  } else {
    res.status(400).send('Invalid session');
  }
});

app.delete('/mcp', async (req, res) => {
  const sessionId = req.headers['mcp-session-id'];
  const transport = transports[sessionId];
  if (transport) {
    await transport.handleRequest(req, res);
  } else {
    res.status(400).send('Invalid session');
  }
});

app.listen(PORT, '127.0.0.1', () => {
  console.log(`NASA MCP Server listening on http://127.0.0.1:${PORT}/mcp`);
});
