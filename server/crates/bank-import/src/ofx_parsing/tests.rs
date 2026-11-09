use super::*;

/// A synthetic OFX 1.02 credit-card statement for the fictional
/// household: one transaction with bank-specific tags before FITID, one
/// with escaped ampersands in NAME and FITID.
const SGML: &str = "OFXHEADER:100\r\nDATA:OFXSGML\r\nVERSION:102\r\nSECURITY:NONE\r\n\
ENCODING:USASCII\r\nCHARSET:1252\r\nCOMPRESSION:NONE\r\nOLDFILEUID:NONE\r\nNEWFILEUID:NONE\r\n\r\n\
<OFX><SIGNONMSGSRSV1><SONRS><STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<DTSERVER>20260930120000<LANGUAGE>ENG</SONRS></SIGNONMSGSRSV1>\
<CREDITCARDMSGSRSV1><CCSTMTTRNRS><TRNUID>1<STATUS><CODE>0<SEVERITY>INFO</STATUS>\
<CCSTMTRS><CURDEF>USD<CCACCTFROM><ACCTID>4000123412341234</CCACCTFROM>\
<BANKTRANLIST><DTSTART>20260901<DTEND>20260930\
<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260905120000\n<TRNAMT>-42.50\n\
<MERCHCAT>BAKERIES\n<MCC>5462\n<CARDNAME>ALEX EXAMPLE\n<FITID>ID-0001\n<NAME>CORNER BAKERY\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260906120000\n<TRNAMT>-19.99\n\
<FITID>ID-0002-A&amp;amp;B\n<NAME>SALT &amp; PEPPER DINER\n<MEMO>A &lt;B&gt; &#38; C ^ D\n</STMTTRN>\n\
<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260907120000\n<TRNAMT>0.00\n\
<FITID>ID-0003\n<NAME>CRÈME BRÛLÉE CAFÉ\n</STMTTRN>\n\
</BANKTRANLIST><LEDGERBAL><BALAMT>-62.49<DTASOF>20260930</LEDGERBAL>\
</CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>";

fn transactions(p: &Parsed) -> &[ofx_rs::aggregates::StatementTransaction] {
    let cc = p.document.credit_card().expect("credit card section");
    let resp = cc.statement_responses()[0].response().expect("statement");
    resp.transaction_list()
        .expect("transactions")
        .transactions()
}

#[test]
fn unknown_tags_are_lifted_not_fatal() {
    let p = parse(SGML.as_bytes().to_vec()).expect("parses");
    let t = transactions(&p);
    assert_eq!(t.len(), 3);
    assert_eq!(restore(t[0].fit_id().as_str()), "ID-0001");
    assert_eq!(restore(t[0].name().expect("name")), "CORNER BAKERY");
    let ext = &p.extensions["ID-0001"];
    let tags: Vec<&str> = ext.iter().map(|e| e.tag.as_str()).collect();
    assert_eq!(tags, ["MERCHCAT", "MCC", "CARDNAME"]);
    assert_eq!(ext[1].value, "5462");
}

#[test]
fn entities_are_decoded_once_and_spacing_kept() {
    let p = parse(SGML.as_bytes().to_vec()).expect("parses");
    let t = &transactions(&p)[1];
    assert_eq!(restore(t.name().expect("name")), "SALT & PEPPER DINER");
    assert_eq!(restore(t.memo().expect("memo")), "A <B> & C ^ D");
    // Double-escaped by the bank: decoded once, like other OFX libraries.
    assert_eq!(restore(t.fit_id().as_str()), "ID-0002-A&amp;B");
    assert!(!p.extensions.contains_key("ID-0002-A&amp;B"));
}

#[test]
fn amounts_stay_exact() {
    let p = parse(SGML.as_bytes().to_vec()).expect("parses");
    let t = transactions(&p);
    assert_eq!(t[1].amount().as_decimal().to_string(), "-19.99");
}

#[test]
fn non_ascii_text_survives() {
    let p = parse(SGML.as_bytes().to_vec()).expect("parses");
    assert_eq!(
        restore(transactions(&p)[2].name().expect("name")),
        "CRÈME BRÛLÉE CAFÉ"
    );
}

#[test]
fn escape_round_trips() {
    let s = "a&b^c É ñ 😀 &amp; ^41;";
    assert_eq!(unescape(&escape(s)), s);
    assert!(escape(s).is_ascii());
}

#[test]
fn decode_entities_leaves_unknown_ones() {
    assert_eq!(decode_entities("a &nbsp; b &amp c"), "a &nbsp; b &amp c");
    assert_eq!(decode_entities("&#x41;&#66;"), "AB");
}
