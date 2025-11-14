FROM node:22-slim

WORKDIR /app

# Install curl for health checks and npm
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
RUN npm install -g npm@latest

# Install pctx globally
RUN npm install -g @portofcontext/pctx

# Copy package files and install dependencies
COPY package.json package-lock.json /app/
RUN npm install

# Copy NASA MCP server
COPY nasa-mcp-server.js /app/nasa-mcp-server.js

# Copy pctx configuration
COPY pctx.json /app/pctx.json

# Create startup script that runs both services
COPY start.sh /app/start.sh
RUN chmod +x /app/start.sh

# Set environment variables
ENV NODE_ENV=production
ENV NASA_MCP_PORT=3000
ENV PCTX_PORT=8080

# Expose only pctx port
EXPOSE 8080

# Run the startup script
CMD ["/app/start.sh"]
