#!/usr/bin/env ruby
# runtime/spinel/param_builder.rb held to Rails' own answers
# (tests/params_vectors/params_vectors.json), under CRuby. Driven by
# tests/spinel_param_builder.rs, which also compiles the same check with
# spinel (tests/params_vectors/spinel_driver.rb) from the case file this
# writes with `--cases OUT`.
#
#   ruby tests/spinel_param_builder.rb .
require "json"
root = File.expand_path(ARGV[0] || ".")
load "#{root}/runtime/spinel/param_builder.rb"
load "#{root}/tests/params_vectors/canon.rb"

vectors = JSON.parse(File.read("#{root}/tests/params_vectors/params_vectors.json"), max_nesting: false)

if (i = ARGV.index("--cases"))
  # One case per line: input TAB expected. No vector input holds a tab
  # or a newline (the generator's alphabet has neither); say so if one
  # ever does rather than writing a line the reader splits wrong.
  File.open(ARGV[i + 1], "w") do |f|
    vectors.each do |c|
      abort "vector input holds a tab or newline: #{c["input"].inspect}" if c["input"] =~ /[\t\n]/
      f.puts "#{c["input"]}\t#{ParamsCanon.answer(c["output"])}"
    end
  end
  puts "wrote #{vectors.size} cases"
  exit
end

pass = 0
failures = []
vectors.each do |c|
  want = ParamsCanon.answer(c["output"])
  got = ParamsCanon.answer(ParamBuilder.from_query_string(c["input"].dup))
  if got == want
    pass += 1
  else
    failures << [c["input"], want, got]
  end
end
failures.first(15).each do |input, want, got|
  puts "FAIL #{input.inspect}\n     want #{want[0, 160]}\n     got  #{got[0, 160]}"
end
puts "#{pass}/#{vectors.size} match Rails"
puts "done"
