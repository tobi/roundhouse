# VENDORED from basecamp/once-campfire-rust (MIT), crates/db/ruby/scenario.rb at
# 080f9035141ac09e1db81476a1b229ed24f08d8f: the Rails half of their model differential, run
# here by scripts/campfire-db-differential against the Rails oracle. The emit half is the SAME
# statements, generated into an overlay controller by that script (fixture labels resolved to
# ids) and transpiled with the app.
#
# Runs the scenario mirrored by `src/tests/differential_test.rs` against a database the
# reference app loaded with `bin/rails db:fixtures:load`, for
# `CAMPFIRE_RUBY_SCENARIO_DB=... cargo test -p campfire_db scenario_matches_ruby -- --ignored`.
#
#   RAILS_ENV=test bin/rails db:prepare db:fixtures:load
#   cp storage/db/test.sqlite3 /tmp/fixtures_ruby.sqlite3     # for fixtures_match_ruby_row_for_row
#   RAILS_ENV=test bin/rails runner path/to/scenario.rb
#   sqlite3 storage/db/test.sqlite3 "PRAGMA wal_checkpoint(TRUNCATE)"
ActiveJob::Base.queue_adapter = :test
def fx(model, label) = model.find(ActiveRecord::FixtureSet.identify(label))
david, jason, jz, kevin = %i[david jason jz kevin].map { fx(User, _1) }
Current.user = david

m = fx(Room, :designers).messages.create!(body: "Hello <b>there</b>", client_message_id: "s1", creator: david)
b = m.boosts.create!(content: "hi", booster: jason)
m.boosts.create!(content: "yo", booster: kevin)
b.destroy!
m.update!(body: "Edited hovercraft")
m2 = fx(Room, :hq).messages.create!(body: "Doomed", client_message_id: "s2", creator: jason)
m2.boosts.create!(content: "x", booster: david)
m2.destroy!
fx(Membership, :kevin_designers).destroy
u = User.create!(name: "User", email_address: "u@example.com", password: "secret123456")
Rooms::Open.create!(name: "Open!", creator: david)
Rooms::Closed.create_for({ name: "Hello!", creator: david }, users: [kevin, david])
r = fx(Room, :watercooler).becomes!(Rooms::Open); r.save!
fx(Room, :pets).update!(name: "Pets2")
fx(Membership, :jason_pets).update!(involvement: "invisible")
mm = fx(Membership, :david_hq); mm.connected; mm.connected; mm.disconnected
Rooms::Direct.find_or_create_for([jz, kevin])
kevin.sessions.create!(ip_address: "8.8.8.8", user_agent: "x")
kevin.ban
kevin.unban
jz.remove_banned_content
jz.deactivate
12.times { |n| david.searches.record("q#{n}") }
bot = User.create_bot!(name: "Bot", webhook_url: "http://x")
bot.update_bot!(name: "Bot2", webhook_url: "http://y")
a = Account.first; a.settings.restrict_room_creation_to_administrators = true; a.save!
fx(Room, :hq).messages.create!(body: "Last one", client_message_id: "s3", creator: kevin)
puts "ok"
