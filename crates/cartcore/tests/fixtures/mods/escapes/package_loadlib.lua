local lib = package.loadlib("/usr/lib/libSystem.dylib", "system")
return { { key = "lib", type = "toggle", default = lib ~= nil } }
