use super::Source;
use crate::schema::podcasts;
use diesel::prelude::*;
use rss;

#[derive(Queryable, Identifiable, AsChangeset, Associations, PartialEq)]
#[diesel(belongs_to(Source, foreign_key = source_id))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = podcasts)]
#[derive(Debug, Clone)]
/// Diesel Model of the shows table.
pub struct Podcast {
    id: i32,
    title: String,
    link: String,
    description: String,
    image_uri: Option<String>,
    image_cached: chrono::NaiveDateTime,
    source_id: i32,
}

impl Podcast {
    /// The row ID of this podcast
    pub fn id(&self) -> i32 {
        self.id
    }
    /// The title for this podcast
    pub fn title(&self) -> &str {
        self.title.as_ref()
    }
    /// The link of this podcast
    pub fn link(&self) -> &str {
        self.link.as_ref()
    }
    /// The description of this podcast
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }
    /// The URI of the image for this episode
    pub fn image_uri(&self) -> Option<&str> {
        self.image_uri.as_deref()
    }
    /// The date that the image was cached
    pub fn image_cached(&self) -> &chrono::NaiveDateTime {
        &self.image_cached
    }
    /// The ID for the `Source` foreign key
    pub fn source_id(&self) -> i32 {
        self.source_id
    }
}

///
#[derive(Insertable, AsChangeset)]
#[diesel(table_name = podcasts)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewPodcast {
    title: String,
    link: String,
    description: String,
    image_uri: Option<String>,
    image_cached: Option<chrono::NaiveDateTime>,
    source_id: i32,
}

impl TryFrom<(&rss::Channel, &Source)> for NewPodcast {
    type Error = <Self as TryFrom<(&'static rss::Channel, i32)>>::Error;
    fn try_from((channel, source): (&rss::Channel, &Source)) -> Result<Self, Self::Error> {
        Self::try_from((channel, source.id))
    }
}

impl TryFrom<(&rss::Channel, i32)> for NewPodcast {
    type Error = String;
    fn try_from((channel, source_id): (&rss::Channel, i32)) -> Result<Self, Self::Error> {
        let title = channel.title().trim().to_owned();
        let link = channel.link().trim().to_owned();
        let description = channel.description().trim().to_owned();
        let image = channel
            .itunes_ext()
            .and_then(|s| s.image().map(|url| url.trim()))
            .map(|s| s.to_owned());
        let image_uri = image.or_else(|| channel.image().map(|s| s.url().trim().to_owned()));

        Ok(NewPodcast {
            title,
            link,
            description,
            image_uri,
            image_cached: Some(chrono::Utc::now().naive_utc()),
            source_id,
        })
    }
}

impl NewPodcast {
    ///
    pub fn from_rss(
        channel: &rss::Channel,
        source_id: i32,
    ) -> Result<Self, <Self as TryFrom<(&rss::Channel, i32)>>::Error> {
        Self::try_from((channel, source_id))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::*;
    use std::io::BufReader;
    #[test]
    pub(crate) fn parse_podcast() -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open("test-data/feeds/atp.xml")?;
        let channel = rss::Channel::read_from(BufReader::new(file))?;
        let podcast = NewPodcast::try_from((&channel, 0));
        println!("{:?}", podcast);
        let _ = podcast;
        Ok(())
    }
}
