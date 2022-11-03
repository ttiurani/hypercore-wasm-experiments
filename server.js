const Hypercore = require('hypercore');
const RAM = require('random-access-memory')
const Server = require('simple-websocket/server')
const fs = require('fs')
const { pipeline } = require('stream')
const http = require('http')
const p = require('path')
const { Server: StaticServer } = require('node-static')

const PORT = 9000

// Init a demo feed.
const hypercore = new Hypercore((_) => new RAM())
hypercore.on('ready', () => {
  // Append the README.
  const filename = p.join(__dirname, 'README.md')
  const data = fs.readFileSync(filename, 'UTF-8')
  const lines = data.split(/\r?\n/);
  hypercore.append(lines);
})

// Statically serve parent dir.
const file = new StaticServer(__dirname)
const server = http.createServer(function (request, response) {
  // Return the key as JSON.
  if (request.url === '/key') {
    response.setHeader('Content-Type', 'application/json')
    return response.end(JSON.stringify({ key: hypercore.key.toString('hex') }))
  }
  // Register the static file server.
  request.addListener('end', function () {
    file.serve(request, response)
  }).resume()
})
server.on('error', err => console.log('server error:', err.message))

// Create a websocket server on top of the http server.
const websocket = new Server({ server })
websocket.on('connection', function (socket) {
  console.log('websocket connection open')
  // Pipe out hypercore's replication stream into the websocket.
  const stream = hypercore.replicate(false)
  pipeline(stream, socket, stream, err => {
    console.log('replication error', err.message)
  })
  socket.on('close', () => console.log('websocket connection closed'))
  socket.on('error', err => console.log('websocket error:', err.message))
})
websocket.on('error', err => console.log('websocket server error:', err.message))

// Start listening for connections.
server.listen(PORT, () => {
  console.log(`Listening on http://localhost:${PORT}`)
})
