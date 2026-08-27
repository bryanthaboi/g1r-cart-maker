local socket = require("socket")
return { { key = "net", type = "toggle", default = socket ~= nil } }
