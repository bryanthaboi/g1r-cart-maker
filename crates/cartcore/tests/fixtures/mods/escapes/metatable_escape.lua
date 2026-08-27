local dump = getmetatable("").__index.dump
return { { key = "dump", type = "toggle", default = dump ~= nil } }
