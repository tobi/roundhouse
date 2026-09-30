# Compares two campfire databases table by table, with the values that
# are random or clock-dependent reduced to their SHAPE — the
# normalization once-campfire-rust's differential test applies
# (crates/db/src/tests/differential_test.rs `normalized_dump`), ported:
#
#   password_digest            bcrypt:<first 4 chars>   ($2a$ and the cost)
#   bot_token, token, join_code random:<length>
#   a deactivated email        <local>-deactivated-<uuid>@<domain>
#   *_at                       time:<fraction digits>
#
#   ruby tests/db_differential/compare.rb RAILS_DB EMIT_DB
#
# Exit 1 on any difference, printing each table's rows only one side has.
require "json"
require "open3"

TABLES = [
  ["accounts", "id"],
  ["action_text_rich_texts", "id"],
  ["bans", "id"],
  ["boosts", "id"],
  ["memberships", "room_id, user_id"],
  ["messages", "id"],
  ["push_subscriptions", "id"],
  ["rooms", "id"],
  ["searches", "id"],
  ["sessions", "id"],
  ["users", "id"],
  ["webhooks", "id"],
  ["message_search_index", "rowid"],
].freeze

def rows(db, table, order)
  columns = table == "message_search_index" ? "rowid AS id, body" : "*"
  out, err, status = Open3.capture3("sqlite3", "-json", db, %(SELECT #{columns} FROM "#{table}" ORDER BY #{order}))
  abort "sqlite3 #{db} #{table}: #{err}" unless status.success?
  out.strip.empty? ? [] : JSON.parse(out)
end

def shape(name, value)
  return value.inspect unless value.is_a?(String)
  case name
  when "password_digest" then "bcrypt:#{value[0, 4]}"
  when "bot_token", "token", "join_code" then "random:#{value.length}"
  when "email_address"
    if (m = value.match(/\A(.*)-deactivated-([^@]*)(@.*)\z/))
      "#{m[1]}-deactivated-<uuid:#{m[2].length}>#{m[3]}"
    else
      value.inspect
    end
  else
    name.end_with?("_at") ? "time:#{value.split(".", 2)[1].to_s.length}" : value.inspect
  end
end

def normalized(db, table, order)
  rows(db, table, order).map do |row|
    row.sort.map { |name, value| "#{name}=#{shape(name, value)}" }.join(" ")
  end
end

# KNOWN DIVERGENCES, each owned by a docs/pipeline/runtime.md entry and
# printed when applied: a table that differs ONLY in the ways named here
# passes as forgiven; any other difference still fails. Retire an entry
# when its runtime.md entry closes.
FORGIVEN = {
  # `insert_all` runs one INSERT per row with the runtime's own
  # timestamps (microseconds) where Rails issues one statement stamped by
  # SQLite (milliseconds) — so the rows `grant_membership_to_open_rooms`
  # writes carry other ids and another precision, and nothing else.
  "memberships" => {
    ledger: "runtime.md: `insert_all` runs save callbacks and issues one INSERT per row",
    normalize: ->(row) { row.gsub(/\bid=\d+ /, "").gsub(/time:\d/, "time") },
  },
}.freeze

rails_db, emit_db = ARGV
abort "usage: compare.rb RAILS_DB EMIT_DB" unless rails_db && emit_db

differing = 0
TABLES.each do |table, order|
  expected = normalized(rails_db, table, order)
  actual = normalized(emit_db, table, order)
  if expected == actual
    puts "ok    #{table} (#{expected.size} rows)"
    next
  end
  if (f = FORGIVEN[table]) && expected.map(&f[:normalize]).sort == actual.map(&f[:normalize]).sort
    puts "ok*   #{table} (#{expected.size} rows; known divergence forgiven — #{f[:ledger]})"
    next
  end
  differing += 1
  puts "DIFF  #{table} (rails #{expected.size} rows, emit #{actual.size})"
  (expected - actual).each { |r| puts "  rails only: #{r[0, 400]}" }
  (actual - expected).each { |r| puts "  emit only:  #{r[0, 400]}" }
  if (expected - actual).empty? && (actual - expected).empty?
    puts "  same rows, different order"
  end
end
puts "#{differing} table(s) differ"
exit(differing.zero? ? 0 : 1)
