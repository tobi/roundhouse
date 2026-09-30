# VENDORED from basecamp/once-campfire-rust (MIT), crates/kit/tests/params_vectors.rb at
# 080f9035141ac09e1db81476a1b229ed24f08d8f. Run with the Rails oracle's bundle, it reproduces
# params_vectors.json byte for byte:
#
#   (cd build/campfire-oracle && bundle exec ruby ../../tests/params_vectors/params_vectors.rb) \
#     > tests/params_vectors/params_vectors.json
#
# Generates tests/params_vectors.json: query strings and what
# ActionDispatch::ParamBuilder.from_query_string makes of them (null when Rails raises, which
# it answers with a 400).
#
#   docker run --rm -v "$PWD/crates/kit/tests:/out" campfire-reference \
#     bundle exec ruby /out/params_vectors.rb > crates/kit/tests/params_vectors.json
require "rails"
require "action_controller/railtie"
require "json"

HANDWRITTEN = [
  "", "a", "a=", "a=1", "a=1&a=2", "&&a=1&", "a=1&  b=2", "=1", "a=b=c", "a;b=1",
  "a=x+y%20z", "a%5Bb%5D=1", "caf%C3%A9=%E2%9C%93", "a=%", "a=%zz", "a=%FF", "=%FF", "%FF=1",
  "a[b]=1", "a[b][c]=1&a[b][d]=2", "a[b]c=1", "[a]=1", "a[=1", "a[b=1", "a]=1", "a[]]=1", "a[[]]=1",
  "a[]=1&a[]=2", "a[]", "a[]=&a[]", "a[][]=1", "a[][]", "a[b][]=1&a[b][]=2",
  "a[][b]=1&a[][c]=2&a[][b]=3", "a[][b][c]=1&a[][b][d]=2", "a[][b][c]=1&a[][b][c]=2",
  "a[][b][]=1&a[][b][]=2", "a[]b=1", "a[][b]", "a&a[]=1", "a&a[b]=1",
  "a=1&a[]=2", "a=1&a[b]=2", "a[]=1&a[b]=2", "a[b]=1&a[]=2", "a[b]=1&a[b][c]=2",
  "user[name]=Jo&user[tags][]=a&user[settings][x][y]=1", "x[y][z][]=1&x[y][z][]=2&x[y][w]=3",
  "a[][b][c]=1&a[][b][d]=2&a[][e]=3", "a[][b]=1&a[][b][c]=2", "a[x][][y]=1&a[x][][z]=2",
  "a[][]=1&a[][]=2", "a[][][b]=1&a[][][c]=2", "a[b][[c]]=1", "a[b]]]=1", "a[]x[y]=1",
  "_method=patch&authenticity_token=abc%3D%3D", "q=%E2%9C%93+check&page=2",
  "a" + "[b]" * 99 + "=1", "a" + "[b]" * 100 + "=1",
]

ALPHABET = ["a", "b", "c", "[", "]", "[]", "[b]", "[][c]", "=", "&", "%5B", "%5D", "%", "%41", "x", " ", "+", "1", ";", "=v"]
rng = Random.new(20240601)
generated = 3000.times.map do
  Array.new(rng.rand(1..12)) { ALPHABET[rng.rand(ALPHABET.size)] }.join
end

vectors = (HANDWRITTEN + generated).uniq.map do |qs|
  output = begin
    JSON.parse(JSON.generate(ActionDispatch::ParamBuilder.from_query_string(qs), max_nesting: false), max_nesting: false)
  rescue ActionDispatch::ParamError, Rack::Utils::InvalidParameterError, Rack::Utils::ParameterTypeError,
         Rack::QueryParser::ParamsTooDeepError, ArgumentError, JSON::GeneratorError, EncodingError
    nil
  end
  { "input" => qs, "output" => output }
end

puts JSON.pretty_generate(vectors, max_nesting: false)
