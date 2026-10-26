Delta = Struct.new(:content, :tool_calls)
Choice = Struct.new(:finish_reason, :delta)
Chunk = Struct.new(:choices)

class AssertionFailure < StandardError
end

class Expectation
  def initialize(value)
    @value = value
  end

  def to(expected)
    raise AssertionFailure, "#{@value.inspect} != #{expected.inspect}" unless @value == expected
  end

  def not_to(expected)
    raise AssertionFailure, "#{@value.inspect} == #{expected.inspect}" if @value == expected
  end
end

def expect(value) = Expectation.new(value)
def be(value) = value
def be_nil = nil

def chunk(reason = nil, text = "", tools = [])
  Chunk.new([Choice.new(reason, Delta.new(text, tools))])
end

def boundary(chunks)
  # GENERATED_ASSERTION ~keep
  true
rescue AssertionFailure
  false
end

terminal = chunk("stop")
empty = Chunk.new([])
cases = [
  [[terminal], true],
  [[chunk(nil, "text"), terminal], true],
  [[empty, terminal], true],
  [[terminal, empty], false],
  [[terminal, chunk], false],
  [[terminal, chunk(nil, "late")], false],
  [[terminal, chunk(nil, "", ["tool"])], false],
  [[terminal, terminal], false],
  [[chunk(nil, "unterminated")], false],
  [[], false]
]
cases.each_with_index do |(chunks, expected), index|
  actual = boundary(chunks)
  raise "case #{index}: #{actual} != #{expected}" unless actual == expected
end

chunks = [terminal, empty]
done_at = chunks.index { |c| c.choices.any? { |ch| !ch.finish_reason.nil? } }
old = !done_at.nil? &&
  chunks.drop(done_at + 1).all? do |c|
    c.choices.all? { |ch| ch.delta.content.to_s.empty? && (ch.delta.tool_calls || []).empty? }
  end
raise "old predicate must demonstrate an empty-chunk false pass" unless old
raise "generated strict predicate must reject the same stream" if boundary(chunks)
puts("BOUNDARY_CASES=#{cases.length};OLD_PREDICATE_FALSE_PASS=1")
