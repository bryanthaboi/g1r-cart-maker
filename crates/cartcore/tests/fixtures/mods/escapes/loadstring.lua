local chunk = (loadstring or load)("return io.open")
return { { key = "chunk", type = "toggle", default = chunk() ~= nil } }
