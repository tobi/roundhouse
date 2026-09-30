# Boots Rails campfire on a database the EMIT wrote — the scenario run
# through the transpiled models — and reads, authenticates, searches,
# edits and deletes through Active Record, the way Rails would after a
# rollback from the emit. Adapted from once-campfire-rust's
# crates/db/ruby/rollback.rb (MIT) to the rows scenario.rb leaves behind.
#
#   DATABASE_URL=sqlite3:<emit db> bin/rails runner tests/db_differential/rollback.rb
ActiveJob::Base.queue_adapter = :test
CHECKS = []
def check(what)
  ok = begin
    yield
  rescue => e
    puts "  raised #{e.class}: #{e.message[0, 200]}"
    false
  end
  CHECKS << [what, ok]
  puts "#{ok ? "ok  " : "FAIL"} #{what}"
end

message = Message.find_by(client_message_id: "s1")
check("the emit's message is there") { !message.nil? }
if message
  check("its body") { message.plain_text_body == "Edited hovercraft" }
  check("its room and creator") { message.room.name == "Designers" && message.creator.name == "David" }
  check("its one remaining boost") { message.boosts.sole.then { _1.content == "yo" && _1.booster.name == "Kevin" } }
  check("search finds it (the emit's FTS rows)") { Message.search("hovercraft").include?(message) }
end
check("the doomed message and its boost are gone") { Message.find_by(client_message_id: "s2").nil? }

user = User.find_by(email_address: "u@example.com")
check("the emit's user authenticates (its bcrypt)") { user&.authenticate("secret123456") }
check("the watercooler (\"All Talk\") became an open room") { Room.find_by(name: "All Talk")&.is_a?(Rooms::Open) }
closed = Room.find_by(name: "Hello!")
check("the closed room and its members") { closed.is_a?(Rooms::Closed) && closed.users.pluck(:name).sort == %w[David Kevin] }
check("the direct room") { Rooms::Direct.all.any? { |r| r.users.pluck(:name).sort == %w[JZ Kevin] } }
# The ban took his sessions with it; the unban lifted the ban.
check("kevin: banned, then unbanned") { User.find_by(name: "Kevin").then { |k| k.active? && k.sessions.none? && Ban.none? } }
check("jz is deactivated") { User.find_by(name: "JZ").deactivated? }
check("david's last ten searches") { User.find_by(name: "David").searches.pluck(:query).sort == (2..11).map { "q#{_1}" }.sort }
check("the bot and its webhook") { User.find_by(name: "Bot2")&.webhook&.url == "http://y" }
check("account settings") { Account.first.settings.restrict_room_creation_to_administrators == true }

if message
  Current.user = User.find_by(name: "David")
  message.update!(body: "Edited by Rails zeppelin")
  check("a Rails edit is searchable") { Message.search("zeppelin").include?(message) && Message.search("hovercraft").exclude?(message) }
  message.destroy!
  check("a Rails delete clears the index and boosts") { Message.search("zeppelin").none? && Boost.where(message_id: message.id).none? }
end

failed = CHECKS.count { |_, ok| !ok }
puts "rollback: #{CHECKS.size - failed}/#{CHECKS.size}"
exit(failed.zero? ? 0 : 1)
