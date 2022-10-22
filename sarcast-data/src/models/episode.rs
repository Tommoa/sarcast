use super::Podcast;
use crate::schema::episodes;
use diesel::prelude::*;
use rss;

#[derive(Queryable, Identifiable, AsChangeset, Associations, PartialEq)]
#[diesel(table_name = episodes)]
#[diesel(treat_none_as_null = true)]
#[diesel(primary_key(title, podcast_id))]
#[diesel(belongs_to(Podcast, foreign_key = podcast_id))]
#[derive(Debug, Clone)]
/// Diesel Model of the episode table.
pub struct Episode {
    title: String,
    uri: Option<String>,
    local_uri: Option<String>,
    description: Option<String>,
    epoch: i32,
    length: Option<i32>,
    duration: Option<i32>,
    guid: Option<String>,
    played: Option<i32>,
    play_position: i32,
    podcast_id: i32,
}

impl Episode {
    /// The title of this episode
    pub fn title(&self) -> &str {
        &self.title
    }
    /// The URI to the playable media for this episode
    pub fn uri(&self) -> Option<&str> {
        self.uri.as_deref()
    }
    /// The local URI to the playable media for this episode
    pub fn local_uri(&self) -> Option<&str> {
        self.local_uri.as_deref()
    }
    /// The description of this episode
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    /// The epoch that this episode was published at
    pub fn epoch(&self) -> i32 {
        self.epoch
    }
    /// The length of the podcast in bytes
    pub fn length(&self) -> Option<i32> {
        self.length
    }
    /// The duration of the episode in milliseconds
    pub fn duration(&self) -> Option<i32> {
        self.duration
    }
    /// The episode's `guid`
    pub fn guid(&self) -> Option<&str> {
        self.guid.as_deref()
    }
    /// When this episode was last played
    pub fn played(&self) -> Option<i32> {
        self.played
    }
    /// The position that this episode is last in for playing
    pub fn play_position(&self) -> i32 {
        self.play_position
    }
    /// The ID of the show that this episode is from
    pub fn podcast_id(&self) -> i32 {
        self.podcast_id
    }
}

///
#[derive(Insertable, AsChangeset)]
#[diesel(table_name = episodes)]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewEpisode {
    title: String,
    uri: Option<String>,
    description: Option<String>,
    length: Option<i32>,
    duration: Option<i32>,
    play_position: i32,
    guid: Option<String>,
    epoch: i32,
    podcast_id: i32,
}

impl TryFrom<(&rss::Item, &Podcast)> for NewEpisode {
    type Error = <Self as TryFrom<(&'static rss::Item, i32)>>::Error;
    fn try_from((item, show): (&rss::Item, &Podcast)) -> Result<Self, Self::Error> {
        Self::try_from((item, show.id()))
    }
}

impl TryFrom<(&rss::Item, i32)> for NewEpisode {
    type Error = String;
    fn try_from((item, podcast_id): (&rss::Item, i32)) -> Result<Self, Self::Error> {
        if item.title().is_none() {
            return Err("No title specified for this Episode.".into());
        }

        let title = item.title().unwrap().trim().to_owned();
        let guid = item.guid().map(|s| s.value().trim().to_owned());

        // Get the mime type, the `http` url and the length from the enclosure
        // http://www.rssboard.org/rss-specification#ltenclosuregtSubelementOfLtitemgt
        let enc = item.enclosure();

        // Get the url
        let uri = enc
            .map(|s| s.url().trim().to_owned())
            // Fallback to Rss.Item.link if enclosure is None.
            .or_else(|| item.link().map(|s| s.trim().to_owned()));

        // Get the size of the content, it should be in bytes
        let length = enc.and_then(|x| x.length().parse().ok());

        // If url is still None return an Error as this behaviour is not
        // compliant with the RSS Spec.
        if uri.is_none() {
            return Err("No url specified for the item.".into());
        };

        // Default to rfc2822 representation of epoch 0.
        let date = chrono::DateTime::parse_from_rfc2822(
            item.pub_date().unwrap_or("Thu, 1 Jan 1970 00:00:00 +0000"),
        );
        // If the date is invalid, just take the 0 epoch
        let epoch = date.map(|x| x.timestamp() as i32).unwrap_or(0);

        let description = item.description().map(|s| s.to_owned());

        Ok(NewEpisode {
            title,
            uri,
            length,
            duration: None,
            play_position: 0,
            description,
            epoch,
            guid,
            podcast_id,
        })
    }
}

impl NewEpisode {
    ///
    pub fn from_rss(
        item: &rss::Item,
        podcast_id: i32,
    ) -> Result<Self, <Self as TryFrom<(&rss::Item, i32)>>::Error> {
        Self::try_from((item, podcast_id))
    }
}
