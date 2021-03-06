// Implementing using tutorial from https://malloc.fi/using-google-analytics-with-next-js
import ReactGA from "react-ga";
import config from "./config";

export const initGA = () => {
    //console.log("GA init");
    ReactGA.initialize(config.GACode());
};

export const logPageView = () => {
    //console.log(`log page: ${window.location.pathname}`);
    ReactGA.set({page: window.location.pathname});
    ReactGA.pageview(window.location.pathname);
};

export const logEvent = (category = "", action = "") => {
    if (category && action) {
        console.log(`log event: `,{category, action});
        ReactGA.event({category, action});
    }
};

export const logError = (description = "", fatal = false) => {
    if (description) {
        ReactGA.exception({description, fatal});
    }
};
