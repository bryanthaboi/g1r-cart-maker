local bytes = string.dump(function() return 1 end)
return { { key = "bytes", type = "text", default = bytes } }
