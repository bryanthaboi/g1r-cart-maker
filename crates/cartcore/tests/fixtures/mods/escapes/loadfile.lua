local chunk = loadfile("/etc/passwd")
return { { key = "chunk", type = "toggle", default = chunk ~= nil } }
