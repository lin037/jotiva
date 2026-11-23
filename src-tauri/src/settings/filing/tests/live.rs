use super::{DEFAULT_MODEL, Folder, destination, endpoint, post_fixture, request};

#[test]
#[ignore = "uses the supplied HTTPS model service with three synthetic notes"]
fn classifies_synthetic_notes() {
    let url = std::env::var("JOT_JEV_ENDPOINT").expect("set JOT_JEV_ENDPOINT");
    let endpoint = endpoint(&url).unwrap();
    let secret = std::env::var("JOT_JEV_API_KEY").expect("set JOT_JEV_API_KEY");
    assert!(
        !secret.is_empty() && secret.bytes().all(|b| b.is_ascii_graphic()),
        "invalid test credential"
    );
    let folders = [
        (1, "园艺", "阳台种植、浇水、盆栽养护"),
        (2, "烘焙", "面包糕点的配方与烤箱操作"),
    ]
    .map(|(id, name, description)| Folder {
        id,
        name: name.into(),
        description: description.into(),
        classify: true,
        path: Default::default(),
    });
    for (text, expected) in [
        (
            "周六给阳台上的迷迭香换盆。先用疏松的土填好陶盆，再把苗移进去，浇透后放回朝南的窗台。",
            Some(1),
        ),
        (
            "准备明早的核桃面包。今晚先揉面，盖好发酵到两倍大，再整形入模，烤箱预热到一百八十度。",
            Some(2),
        ),
        (
            "周三晚上去剧场听弦乐四重奏。七点在正门取票，演出结束后沿河边步行到地铁站坐末班车回家。",
            None,
        ),
    ] {
        let payload = request(text, &folders, DEFAULT_MODEL).unwrap();
        let (status, reply) = post_fixture(endpoint.as_str(), &secret, &payload);
        assert_eq!(
            status, 200,
            "model service did not return a successful response"
        );
        let choice = destination(&reply, &folders);
        assert_eq!(choice.unwrap(), expected);
    }
}
