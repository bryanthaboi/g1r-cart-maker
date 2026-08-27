local env = debug.getregistry()
return { { key = "env", type = "toggle", default = env ~= nil } }
