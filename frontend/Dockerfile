# frontend build → node runtime (adapter-node for server-side features)
FROM node:22-alpine AS build
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:22-alpine
WORKDIR /app
COPY --from=build /app/build ./build
COPY --from=build /app/package*.json ./
RUN npm ci --omit=dev
EXPOSE 5173
# adapter-node exposes PORT
ENV PORT=5173
ENV ORIGIN=http://localhost:5173
CMD ["node", "build"]
