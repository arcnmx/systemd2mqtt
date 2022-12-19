macro_rules! impl_entity {
	(@document $($ty:ident = $domain:literal,)*) => { $(
		impl StaticEntity for hass_mqtt_types::$ty<'_> {
			const DOMAIN: &'static str = $domain;
		}

		impl<'i> EntityObject for hass_mqtt_types::$ty<'i> {
			fn unique_id(&self) -> Option<UniqueId> {
				self.unique_id.as_ref().map(|s| s[..].into())
			}

			fn object_id(&self) -> Option<&str> {
				self.object_id.as_ref().map(|s| &s[..])
			}

			fn domain(&self) -> &str {
				Self::DOMAIN
			}
		}

		impl<'i> EntityDocument for hass_mqtt_types::$ty<'i> {
			#[cfg(feature = "gat")]
			type Document<'o> = &'o Self where Self: 'o;

			#[cfg(feature = "gat")]
			fn to_document<'o>(&'o self) -> Self::Document<'o> {
				self
			}
		}
	)* };
	(@wrapper $($ty:ident[$as:ident] = $doc:ident,)*) => { $(
		impl StaticEntity for $ty<'_> {
			const DOMAIN: &'static str = hass_mqtt_types::$doc::DOMAIN;
		}

		impl<'i> EntityObject for $ty<'i> {
			fn unique_id(&self) -> Option<UniqueId> {
				Some(self.unique_id[..].into())
			}

			fn object_id(&self) -> Option<&str> {
				Some(&self.object_id[..])
			}

			fn domain(&self) -> &str {
				Self::DOMAIN
			}
		}

		impl<'i> EntityDocument for $ty<'i> {
			#[cfg(feature = "gat")]
			type Document<'o> = hass_mqtt_types::$doc<'o> where Self: 'o;

			#[cfg(feature = "gat")]
			fn to_document<'o>(&'o self) -> Self::Document<'o> {
				self.$as()
			}
		}

		impl Serialize for $ty<'_> {
			fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
				match () {
					#[cfg(feature = "gat")]
					() => self.to_document().serialize(s),
					#[cfg(not(feature = "gat"))]
					() => self.$as().serialize(s),
				}
			}
		}
	)* };
}
