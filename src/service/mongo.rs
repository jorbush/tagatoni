use crate::model::recipe::Recipe;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use mongodb::Client;

pub struct MongoService {
    client: Client,
    db_name: String,
}

impl MongoService {
    pub fn new(client: Client, db_name: String) -> Self {
        Self { client, db_name }
    }

    /// Fetches a single recipe that has missing calories or missing recipeCuisine.
    /// We search for documents where calories is null or doesn't exist, OR recipeCuisine is null or doesn't exist.
    pub async fn fetch_next_recipe_to_audit(
        &self,
    ) -> Result<Option<Recipe>, mongodb::error::Error> {
        let recipe_collection = self
            .client
            .database(&self.db_name)
            .collection::<Recipe>("Recipe");

        let filter = doc! {
            "$or": [
                { "calories": { "$exists": false } },
                { "calories": null },
                { "recipeCuisine": { "$exists": false } },
                { "recipeCuisine": null },
                { "recipeYield": { "$exists": false } },
                { "recipeYield": null }
            ]
        };

        recipe_collection.find_one(filter).await
    }

    pub async fn update_recipe_seo(
        &self,
        id: &ObjectId,
        calories: i32,
        recipe_cuisine: &str,
        recipe_yield: i32,
    ) -> Result<(), mongodb::error::Error> {
        let recipe_collection = self
            .client
            .database(&self.db_name)
            .collection::<mongodb::bson::Document>("Recipe");

        let filter = doc! { "_id": id };
        let update = doc! {
            "$set": {
                "calories": calories,
                "recipeCuisine": recipe_cuisine,
                "recipeYield": recipe_yield
            }
        };

        recipe_collection.update_one(filter, update).await?;
        Ok(())
    }
}
