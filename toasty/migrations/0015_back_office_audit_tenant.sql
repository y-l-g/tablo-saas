-- Written by hand: the back office acts for no team, but an older Tablo filed its entries under the
-- staff member's own team. `tablo::audit::forget_tenant` deletes every entry filed under an erased
-- team, so an administrator deleting their team would wipe what they did in the back office.
-- Those entries now name no team, as the back office writes them today.
UPDATE "audit_entries" SET "tenant" = NULL WHERE "panel" <> '/app';
