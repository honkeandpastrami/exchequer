"""Bank row IDs (Firefly's external IDs) carried onto migrated transactions."""

from uuid import UUID

from firefly_migrate import fingerprints, transform
from tests.conftest import ACCOUNTS, group, journal


def test_each_row_id_goes_on_the_migrated_accounts_its_journal_touches(m):
    p = transform.plan(
        m,
        ACCOUNTS,
        [
            group(10, "2026-03-01", journal(2, "Expense:Grocer", "80", external_id="F1")),
            # A transfer: the ID could have come from either account's file.
            group(11, "2026-03-02", journal(1, 2, "500", type="transfer", external_id="ab12cd")),
            # Firefly accounts collapsed into one migrated account.
            group(12, "2026-03-03", journal(4, "Expense:Fees", "5", external_id="Z9")),
            group(13, "2026-03-04", journal(2, "Expense:Grocer", "1")),
            group(14, "2026-03-05", journal(2, "Expense:Grocer", "2", external_id="x" * 201)),
        ],
    )
    got = {t.firefly_id: t.fingerprints for t in p.transactions if t.firefly_id}
    assert got == {
        "10": [("Alex · Credit", "F1")],
        "11": [("Alex · Checking", "ab12cd"), ("Alex · Credit", "ab12cd")],
        "12": [("Alex · Brokerage", "Z9")],
        "13": [],
        "14": [],
    }
    assert p.report.counts["bank row IDs (Firefly external IDs) on migrated accounts"] == 4
    assert p.report.counts["external IDs left out (over 200 characters)"] == 1


def test_items_need_the_transaction_and_account_on_the_server(m):
    p = transform.plan(
        m,
        ACCOUNTS,
        [
            group(10, "2026-03-01", journal(2, "Expense:Grocer", "80", external_id="F1")),
            group(11, "2026-03-02", journal(1, 2, "500", type="transfer", external_id="T1")),
            group(12, "2026-03-03", journal(2, "Expense:Grocer", "9", external_id="F2")),
        ],
    )
    t10, t11 = UUID(int=10), UUID(int=11)
    card = UUID(int=2)
    out = fingerprints.Outcome()
    items = fingerprints.items_for(p, {"10": t10, "11": t11}, {"Alex · Credit": card}, out)
    assert [(i.account_id, i.fingerprint, i.transaction_id) for i in items] == [
        (card, "F1", t10),
        (card, "T1", t11),
    ]
    assert out.missing_transactions == 1
    assert out.missing_accounts == {"Alex · Checking"}
