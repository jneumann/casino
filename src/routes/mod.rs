mod admin;
mod auth;
mod bank;
mod blackjack;
mod games;
mod pages;
mod status;
mod wallet;

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(auth::register)
            .service(auth::login)
            .service(auth::me)
            .service(wallet::balance)
            .service(bank::terms)
            .service(bank::borrow)
            .service(bank::repay)
            .service(games::paytable)
            .service(games::spin)
            .service(games::video_poker_paytable)
            .service(games::video_poker_hand)
            .service(games::video_poker_deal)
            .service(games::video_poker_draw)
            .service(blackjack::rules)
            .service(blackjack::hand)
            .service(blackjack::deal)
            .service(blackjack::act)
            .service(status::health)
            .service(status::status)
            .service(
                web::scope("/admin")
                    .service(admin::login)
                    .service(admin::me)
                    .service(admin::change_own_password)
                    .service(admin::list_operators)
                    .service(admin::create_operator)
                    .service(admin::update_operator)
                    .service(admin::reset_operator_password)
                    .service(admin::delete_operator)
                    .service(admin::list_players)
                    .service(admin::update_player)
                    .service(admin::adjust_player_balance)
                    .service(admin::list_adjustments),
            ),
    )
    .service(pages::admin_page)
    .service(pages::users_page)
    .service(pages::status_page);
}
