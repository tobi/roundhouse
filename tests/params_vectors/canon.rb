# One canonical string for a params tree, so a vector's expected output
# (parsed from JSON under CRuby) and the builder's answer (possibly in a
# spinel binary, which has no JSON to compare with) meet as two Strings.
# Length-prefixed, so no value needs escaping; insertion-ordered, as
# Rails' Hash is. "E" is Rails' 400.
module ParamsCanon
  def self.canon(x)
    if x.nil?
      "n"
    elsif x.is_a?(String)
      "s" + x.bytesize.to_s + ":" + x
    elsif x.is_a?(Array)
      out = "a" + x.length.to_s + "["
      x.each { |e| out = out + ParamsCanon.canon(e) }
      out + "]"
    elsif x.is_a?(Hash)
      out = "h" + x.length.to_s + "{"
      x.each { |k, v| out = out + "k" + k.to_s.bytesize.to_s + ":" + k.to_s + ParamsCanon.canon(v) }
      out + "}"
    else
      "?"
    end
  end

  def self.answer(result)
    result.nil? ? "E" : ParamsCanon.canon(result)
  end
end
