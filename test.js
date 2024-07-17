import init, {
    split_flurstuecke_background_worker
} from "./pkg/tnviewer.js";

async function functionThatTakesLongTime(someArgument){
    await init();
    return split_flurstuecke_background_worker(someArgument);
}

onmessage = async function(event){
    let result = await functionThatTakesLongTime(event.data);
    postMessage(result);
};


