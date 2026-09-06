#include <lib/std.mi>

Function int getChannels (); // returning 1 for mono, 2 for stereo, more for multichannel (e.g. 6), -1 for no info available
Function string getBitrate();
Function string getFrequency();
Function updateInfo();

Global Group mainGroup, info_Grp_1, info_Grp_2;
Global Text info_KHZ, info_KBPS, info_SIZE, info_mode2;
Global Layer info_layer_size;
Global Timer waitForReturn, recheck;
Global Boolean infoMode; //true=text, false=normal bitmap font

System.onScriptLoaded(){
	mainGroup = getScriptGroup();
	info_Grp_1 = mainGroup.findObject("fileinfo.mode.1");
	info_Grp_2 = mainGroup.findObject("fileinfo.mode.2");
	info_layer_size = mainGroup.findObject("info.size.layer");
	info_KHZ = mainGroup.findObject("info.khz");
	info_KBPS = mainGroup.findObject("info.kbps");
	info_SIZE = mainGroup.findObject("info.size");
	info_mode2 = mainGroup.findObject("fileinfo.mode.2.text");
	
	if(info_layer_size.isInvalid()){
		infoMode=true;
		info_Grp_2.show();
	}
	else{
		infoMode=false;
		info_Grp_1.show();
	}
	
	recheck = new Timer;
	recheck.setDelay(1000);
	
	waitForReturn = new Timer;
	waitForReturn.setDelay(100);
	updateInfo();
	

}
System.onscriptunloading(){
	delete waitForReturn;
	delete recheck;
}

System.onTitleChange(String newtitle){
	recheck.stop();
	updateInfo();
}

waitForReturn.onTimer(){
	updateInfo();
}

System.onStop(){
	recheck.stop();
}
System.onPlay(){
	recheck.start();
}
System.onPause(){
	recheck.stop();
}
System.onResume(){
	recheck.start();
}
recheck.onTimer(){
	updateInfo();
}

updateInfo(){
	if(infoMode){
		String output = ""; 
		
		output += getBitrate()+"kbps    "+integerToString(stringToInteger(getFrequency()))+"khz    ";
	
		if(getChannels() > 2) output+="Surround";
		else if(getChannels() == 2) output+="Stereo";
		else output+="Mono";


		info_mode2.setText(output);
	}
	else{
		info_KHZ.setText(getBitrate());
		info_KBPS.setText(integerToString(stringToInteger(getFrequency())));
		//setClipboardText(strmid(System.getPlayItemString(), 7, strlen(System.getPlayItemString())-7));
		
		float rawsize = System.getFileSize(strmid(System.getPlayItemString(), 7, strlen(System.getPlayItemString())-7));

		if(rawsize<1024*1024){
			rawsize=rawsize/1024; //KB
			info_layer_size.setXmlParam("image", "size.kb");
		}
		else if(rawsize<1024*1024*1024){
			rawsize=rawsize/1024/1024; //MB
			info_layer_size.setXmlParam("image", "size.mb");
		}
		else{
			rawsize=rawsize/1024/1024/1024; //GB
			info_layer_size.setXmlParam("image", "size.gb");
		}
		info_SIZE.setText(floatToString(rawsize,1));
	}

	//Refresh if NA
	String sit = getSongInfoText();
	if (sit != ""){
		waitForReturn.stop();
		recheck.start();
	}
	else if(waitForReturn.isRunning()==false){
		waitForReturn.start();
	}
}

Int getChannels ()
{
	if (strsearch(getSongInfoText(), "tereo") != -1)
	{
		return 2;
	}
	else if (strsearch(getSongInfoText(), "ono") != -1)
	{
		return 1;
	}
	else if (strsearch(getSongInfoText(), "annels") != -1)
	{
		int pos = strsearch(getSongInfoText(), "annels");
		return stringToInteger(strmid(getSongInfoText(), pos - 4, 1));
	}
	else
	{
		return -1;
	}
}

string getBitrate ()
{
	string sit = getSongInfoText();
	if (sit != "")
	{
		string rtn;
		int searchresult;
		for (int i = 0; i < 5; i++) {
			rtn = getToken(sit, " ", i);
			searchResult = strsearch(rtn, "kbps");
			if (searchResult>0){
				if(stringToInteger(StrMid(rtn, 0, searchResult))>99999){
					return "99999";
				}
				return StrMid(rtn, 0, searchResult);
			}
		}
		return "";
	}
	else
	{
		return "";
	}
}

string getFrequency ()
{
	string sit = getSongInfoText();
	if (sit != "")
	{
		string rtn;
		int searchresult;
		for (int i = 0; i < 5; i++) {
			rtn = getToken(sit, " ", i);
			searchResult = strsearch(strlower(rtn), "khz");
			if (searchResult>0) return StrMid(rtn, 0, searchResult);
		}
		return "";
	}
	else
	{
		return "";
	}
}