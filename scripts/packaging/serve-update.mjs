import { createServer } from "node:http"
import { createReadStream, statSync } from "node:fs"
import { join } from "node:path"

const root = "D:/code/rust/tTerm-p0-26-scratch/server"
const files = new Map([
  ["/latest.json", ["latest.json", "application/json"]],
  [
    "/tTerm-GPUI-Dev_0.0.2_x64-setup.exe",
    ["tTerm-GPUI-Dev_0.0.2_x64-setup.exe", "application/octet-stream"],
  ],
])

const server = createServer((request, response) => {
  const entry = files.get(request.url)
  const status = entry && ["GET", "HEAD"].includes(request.method) ? 200 : 404
  console.log(
    JSON.stringify({
      time: new Date().toISOString(),
      method: request.method,
      url: request.url,
      status,
    })
  )
  if (status !== 200) {
    response.writeHead(404).end()
    return
  }
  const [name, type] = entry
  const path = join(root, name)
  response.writeHead(200, {
    "Content-Type": type,
    "Content-Length": statSync(path).size,
    "Cache-Control": "no-store",
  })
  if (request.method === "HEAD") response.end()
  else createReadStream(path).pipe(response)
})
server.listen(18226, "127.0.0.1", () => console.log("Listening on http://127.0.0.1:18226"))
