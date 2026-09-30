# The params vectors under a SPINEL-compiled ParamBuilder. The harness
# (tests/spinel_param_builder.rs) copies runtime/spinel/param_builder.rb
# and canon.rb beside this file, and the case file the CRuby driver
# writes (`input TAB expected-canon` per line) as cases.tsv.
require_relative "param_builder"
require_relative "canon"

lines = File.read(ARGV[0] || "cases.tsv").split("\n")
pass = 0
fail_count = 0
lines.each do |line|
  tab = line.index("\t")
  next if tab.nil?
  input = line[0, tab].to_s
  want = line[tab + 1, line.length - tab - 1].to_s
  got = ParamsCanon.answer(ParamBuilder.from_query_string(input))
  if got == want
    pass += 1
  else
    fail_count += 1
    if fail_count <= 15
      puts "FAIL " + input
      puts "     want " + want[0, 160].to_s
      puts "     got  " + got[0, 160].to_s
    end
  end
end
puts pass.to_s + "/" + lines.length.to_s + " match Rails"
puts "done"
