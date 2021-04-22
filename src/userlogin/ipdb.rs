use warp::hyper::{body, Client, Uri};

#[derive(Debug, Clone, Default)]
pub struct IpDB {
	url: String,
}

impl IpDB {
	///
	#[must_use]
	pub const fn new(url: String) -> Self {
		Self { url }
	}

	///
	pub async fn lookup(&self, ip: &str) -> Option<String> {
		if let Ok(uri) = format!("{}/{}", self.url, ip).parse::<Uri>()
		{
			if let Ok(resp) = Client::new().get(uri).await {
				if let Ok(buf) = body::to_bytes(resp).await {
					if buf.len() == 2 {
						let result =
							String::from(String::from_utf8_lossy(
								buf.slice(0..2).as_ref(),
							));
						return Some(result);
					}
				}
			}
		}

		None
	}
}
