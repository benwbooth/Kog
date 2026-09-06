#include <lib/std.mi>

Function updateChannel(String SongInfoString);
Function string tokenizeSongInfo(String tkn, String sinfo);

Global Group XUIGroup;
Global Layer lay1ch, lay2ch, lay6ch, overlay;
Global Timer songInfoTimer;

System.onScriptLoaded() {
	XUIGroup = getScriptGroup();
	lay1ch = XUIGroup.findObject("mono");
	lay2ch = XUIGroup.findObject("stereo");
	lay6ch = XUIGroup.findObject("6ch");
	overlay = XUIGroup.findObject("sccoverlay");
	
	songInfoTimer = new Timer;
	songInfoTimer.setDelay(2000);

	if (getStatus() == STATUS_PLAYING) {
		String sit = getSongInfoText();
		if (sit != "") updateChannel(sit);
		else songInfoTimer.setDelay(100); // goes to 2000 once info is available
		songInfoTimer.start();
	}
	else if (getStatus() == STATUS_PAUSED) {
		updateChannel(getSongInfoText());
	}
}

System.onScriptUnloading(){
	delete songInfoTimer;
}

System.onSetXuiParam(String param, String value) {
	if(strlower(param) == "ch_1"){
		lay1ch.setXmlParam("image", value);
	}
	else if(strlower(param) == "ch_2"){
		lay2ch.setXmlParam("image", value);
	}
	else if(strlower(param) == "ch_6"){
		lay6ch.setXmlParam("image", value);
	}
	else if(strlower(param) == "ghost_all"){
		lay1ch.setXmlParam("ghost", value);
		lay2ch.setXmlParam("ghost", value);
		lay6ch.setXmlParam("ghost", value);
		overlay.setXmlParam("ghost", value);
	}
	else{
		lay1ch.setXmlParam(param, value);
		lay2ch.setXmlParam(param, value);
		lay6ch.setXmlParam(param, value);
		overlay.setXmlParam(param, value);
	}
}

updateChannel(String SongInfoString){
	String tkn;
	tkn = tokenizeSongInfo("Channels", SongInfoString);
	lay1ch.hide();
	lay2ch.hide();
	lay6ch.hide();

	if(tkn=="1ch" || tkn=="0ch"){
		lay1ch.show();
	}
	else if(tkn=="2ch"){
		lay2ch.show();
	}
	else if(tkn=="6ch"){
		lay6ch.show();
	}
}

String tokenizeSongInfo(String tkn, String sinfo){
	int searchResult;
	String rtn;
	if (tkn=="Channels"){
		for (int i = 0; i < 5; i++) {
			rtn = getToken(sinfo, " ", i);
			searchResult = strsearch(rtn, "tereo");
			if (searchResult>0) return "2ch";
			searchResult = strsearch(rtn, "ono");
			if (searchResult>0) return "1ch";
			// Martin: surround > 3, stereo = 2,3
			searchResult = strsearch(rtn, "annels");
			if (searchResult>0)
			{
				int pos = strsearch(getSongInfoText(), "annels");
				pos = stringToInteger(strmid(getSongInfoText(), pos - 4, 1));
				if (pos > 3) return "6ch";
				if (pos > 1 && pos < 4) return "2ch";
				else return "1ch";
			}
		}
		return "0ch";
	}
	else return "";
}

System.onPlay(){
	String sit = getSongInfoText();
	if (sit != "") updateChannel(sit);
	else songInfoTimer.setDelay(100); // goes to 1000 once info is available
	songInfoTimer.start();
}

System.onPause(){
	songInfoTimer.stop();
}

System.onStop(){
	lay1ch.hide();
	lay2ch.hide();
	lay6ch.hide();
	songInfoTimer.stop();
}

songInfoTimer.onTimer(){
	String sit = getSongInfoText();
	if (sit == "") return;
	songInfoTimer.setDelay(2000);
	updateChannel(sit);
}

